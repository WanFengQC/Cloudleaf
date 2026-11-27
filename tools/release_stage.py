"""验证并在临时目录试装固定构建包。"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import sys
import tempfile

from tools.data_pipeline.filesystem import assert_plain_path, assert_plain_tree
from tools.data_pipeline.pipeline import (
    activate as publish,
    atomic_json,
    checked_pointer,
    publish_lock,
    read_json,
    rollback as activate_previous,
)
from tools.data_pipeline.recovery import recover_store

MANIFEST = "manifest.json"
DICTIONARY_SCOPE = "resources/dictionary-lifecycle.json"
DICTIONARY_CATALOG = Path(__file__).resolve().parents[1] / "config/language-resources.json"
ROLES = {"bin", "licenses", "provenance", "resources"}
COMMIT = re.compile(r"[a-f0-9]{40}")
VERSION = re.compile(r"[a-z0-9][a-z0-9.-]{0,47}")
SHA256 = re.compile(r"[a-f0-9]{64}")
USER_CONFIG = ("settings.json", "settings.json.bak", "input-settings.json", "input-settings.json.bak",
               "language-resources.json")
USER_GENERATION = re.compile(r"generation-[0-9]{20}\.clu")


def isolated(path):
    original = path.absolute()
    assert_plain_path(original)
    temporary = Path(tempfile.gettempdir()).resolve()
    resolved = original.resolve()
    if sys.platform == "win32":
        value = str(resolved)
        resolved = Path("\\\\" + value[8:] if value.startswith("\\\\?\\UNC\\") else value.removeprefix("\\\\?\\"))
    if resolved == temporary or not resolved.is_relative_to(temporary):
        raise ValueError("发行演练只能使用系统临时目录的独立子目录")
    if sys.platform == "win32":
        # 内容摘要目录和合法许可树结合时能超过 Win32 的传统路径限制。
        value = str(resolved).removeprefix("\\\\?\\")
        return Path("\\\\?\\UNC\\" + value[2:] if value.startswith("\\\\") else "\\\\?\\" + value)
    return resolved


def file_record(root, path):
    relative = path.relative_to(root).as_posix()
    parts = PurePosixPath(relative).parts
    if (len(parts) < 2 or parts[0] not in ROLES
            or any(not re.fullmatch(r"[A-Za-z0-9_-][A-Za-z0-9_.+-]*", component)
                   or component.endswith(".") for component in parts)
            or path.suffix.lower() in {".safetensors", ".pt", ".pth", ".bin"}
            or path.name.lower() in {"settings.json", "input-settings.json", "user-dictionary.tsv", "credentials.json"}
            or path.name == "language-resources.json" and relative != "resources/config/language-resources.json"):
        raise ValueError(f"发行包路径或资源类型无效：{relative}")
    with path.open("rb") as stream:
        checksum = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"path": relative, "bytes": path.stat().st_size, "sha256": checksum, "role": parts[0]}


def records(root):
    assert_plain_tree(root)
    if not root.is_dir():
        raise ValueError("缺少发行目录")
    paths = (path for path in root.rglob("*") if path.is_file() and path != root / MANIFEST)
    paths = sorted(paths, key=lambda path: path.relative_to(root).as_posix())
    result = [file_record(root, path) for path in paths]
    if len({record["path"].casefold() for record in result}) != len(result):
        raise ValueError("发行包存在大小写冲突")
    if {record["role"] for record in result} != ROLES:
        raise ValueError("发行包缺少工具、许可、溯源或资源目录")
    return result


def dictionary_scope(release, files, proposed=None):
    by_path = {row["path"]: row["sha256"] for row in files}
    material = {}
    for name in by_path:
        if name != DICTIONARY_SCOPE:
            with (release / name).open("rb") as stream:
                if stream.readline(128).rstrip(b"\r\n") == b"# cloudleaf-lexicon-v2":
                    material[name] = by_path[name]
    if DICTIONARY_SCOPE not in by_path:
        if material:
            raise ValueError("发行词典缺少资源身份清单")
        return

    descriptor = read_json(release / DICTIONARY_SCOPE) if proposed is None else proposed
    if (not isinstance(descriptor, dict)
            or set(descriptor) != {"schema_version", "catalog_sha256", "base_payload", "supplement", "user_dictionary"}
            or type(descriptor["schema_version"]) is not int or descriptor["schema_version"] != 1
            or descriptor["supplement"] != {"enabled_by_default": False, "scope": "external"}
            or descriptor["user_dictionary"] != {"generation": None, "scope": "external"}):
        raise ValueError("词典生命周期清单无效")
    catalog_name = "resources/config/language-resources.json"
    if (catalog_name not in by_path or descriptor["catalog_sha256"] != by_path[catalog_name]
            or (release / catalog_name).stat().st_size > 65_536):
        raise ValueError("发行包的词典来源注册表与锁定版本不符")
    catalog = read_json(release / catalog_name)
    trusted = read_json(DICTIONARY_CATALOG)["resources"]
    if (not isinstance(catalog, dict) or set(catalog) != {"schema_version", "resources"}
            or type(catalog["schema_version"]) is not int or catalog["schema_version"] != 1
            or not isinstance(catalog["resources"], list) or not catalog["resources"]
            or any(item not in trusted for item in catalog["resources"])):
        raise ValueError("发行包的词典来源注册表包含未知身份")
    base = descriptor["base_payload"]
    if base is None:
        if material:
            raise ValueError("未声明的词典不得进入发行包")
        return
    if (not isinstance(base, dict)
            or set(base) != {"path", "runtime_sha256", "enabled_by_default"}
            or base["enabled_by_default"] is not True
            or not isinstance(base["path"], str) or not isinstance(base["runtime_sha256"], str)
            or not base["path"].startswith("resources/")
            or material != {base["path"]: base["runtime_sha256"]}):
        raise ValueError("发行包仅允许声明一份经授权的基础词典")
    entry = next((item for item in catalog["resources"] if item["slot"] == "base"
                  and item["admission"] == "approved_source"
                  and item["runtime_sha256"] == base["runtime_sha256"]), None)
    if entry is None:
        raise ValueError("研究或未知词典不得进入发行包")
    source = "resources/" + entry["source_lock"]
    license_id = entry["license_id"]
    if (by_path.get(source) != entry["source_lock_sha256"]
            or by_path.get(f"licenses/data/{license_id}.txt") != entry["license_sha256"]):
        raise ValueError("发行包缺少原件来源锁或许可证")
    notices = [str(PurePosixPath(base["path"]).parent / name) for name in ("NOTICE.txt", "MODIFICATIONS.txt")]
    for name in notices:
        if name not in by_path or (release / name).stat().st_size > 65_536 or not (release / name).read_bytes().strip():
            raise ValueError("基础词典缺少署名或修改说明")


def stamp_dictionaries(release, base_path=None):
    release = isolated(release)
    if (release / MANIFEST).exists() or (release / DICTIONARY_SCOPE).exists():
        raise ValueError("不可重写词典生命周期清单或已经封装的发行包")
    catalog_path = release / "resources/config/language-resources.json"
    assert_plain_path(catalog_path)
    raw = DICTIONARY_CATALOG.read_bytes()
    if not catalog_path.is_file() or catalog_path.read_bytes() != raw:
        raise ValueError("从相同构建获取固定词典来源注册表")
    files = records(release)
    base = None
    if base_path is not None:
        entry = next((row for row in files if row["path"] == base_path), None)
        if entry is None:
            raise ValueError("指定的发布词典文件不存在")
        base = {"path": base_path, "runtime_sha256": entry["sha256"], "enabled_by_default": True}
    data = {"schema_version": 1, "catalog_sha256": hashlib.sha256(raw).hexdigest(),
            "base_payload": base, "supplement": {"enabled_by_default": False, "scope": "external"},
            "user_dictionary": {"generation": None, "scope": "external"}}
    # 先以待发布字节核验同一套条件，失败时不存在已发布描述符。
    dictionary_scope(release, [*files, {"path": DICTIONARY_SCOPE, "sha256": ""}], data)
    atomic_json(release / DICTIONARY_SCOPE, data)
    return data


def verify(release):
    assert_plain_path(release / MANIFEST)
    if (release / MANIFEST).stat().st_size > 2_000_000:
        raise ValueError("发行清单超限")
    manifest = read_json(release / MANIFEST)
    files = records(release)
    if (not isinstance(manifest, dict)
            or set(manifest) != {"schema_version", "scope", "version", "commit", "files"}
            or type(manifest["schema_version"]) is not int
            or manifest["schema_version"] != 1
            or manifest["scope"] != "isolated-rehearsal"
            or not VERSION.fullmatch(str(manifest["version"]))
            or not COMMIT.fullmatch(str(manifest["commit"]))
            or manifest["files"] != files):
        raise ValueError("发行清单、文件身份或范围不匹配")
    dictionary_scope(release, files)
    return manifest


def seal(release, version, commit):
    release = isolated(release)
    if not VERSION.fullmatch(version) or not COMMIT.fullmatch(commit):
        raise ValueError("发行版本或提交无效")
    path = release / MANIFEST
    if path.exists():
        existing = verify(release)
        if (existing["version"], existing["commit"]) == (version, commit):
            return existing
        raise ValueError("不可改写已封装清单")
    files = records(release)
    dictionary_scope(release, files)
    manifest = {"schema_version": 1, "scope": "isolated-rehearsal",
                "version": version, "commit": commit, "files": files}
    atomic_json(path, manifest)
    return verify(release)


def signature_request(release):
    release = isolated(release)
    manifest = verify(release)
    # 签名输入是根清单原始字节。密钥和签名流程由发行方单独维护。
    raw = (release / MANIFEST).read_bytes()
    return {"schema_version": 1, "purpose": "cloudleaf-release-manifest-v1",
            "file": MANIFEST, "signed_input": "raw-manifest-bytes",
            "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
            "version": manifest["version"], "commit": manifest["commit"],
            "signature_status": "unsigned"}


def managed(root):
    root = isolated(root)
    marker = root / "cloudleaf-isolated-stage"
    assert_plain_path(marker)
    if not marker.is_file() or marker.read_bytes() != b"cloudleaf-stage-v1\n":
        raise ValueError("缺少隔离发行根身份")
    return root / "packages"


def initialize(root):
    root = isolated(root)
    marker = root / "cloudleaf-isolated-stage"
    if marker.exists():
        managed(root)
    else:
        root.mkdir(parents=True, exist_ok=True)
        if any(root.iterdir()):
            raise ValueError("初始化只接受空的隔离目录")
        with marker.open("xb") as stream:
            stream.write(b"cloudleaf-stage-v1\n")
    return {"isolated": True}


def install(root, release):
    store = managed(root)
    release = isolated(release)
    if release.is_relative_to(store.parent):
        raise ValueError("安装原件必须在管理根之外")

    def transition(previous, new):
        new_manifest = verify(new)
        if previous is not None:
            old_manifest = verify(previous)
            if old_manifest["version"] == new_manifest["version"] and old_manifest != new_manifest:
                raise ValueError("相同版本不能指向不同构建")

    return publish(release, store, verifier=verify, transition=transition)


def status(root):
    store = managed(root)
    path = store / "current.json"
    assert_plain_path(path)
    if not path.exists():
        return {"active": False}
    state = read_json(path)
    checked_pointer(store, state["current"], verifier=verify)
    return {"active": True, **state}


def uninstall(root):
    store = managed(root)
    with publish_lock(store):
        pointer = store / "current.json"
        retired = store / "last-uninstalled.json"
        assert_plain_path(pointer)
        assert_plain_path(retired)
        if pointer.exists():
            # 原子撤销可见版本。包缓存与用户资料保留，支持审计和离线重装。
            pointer.replace(retired)
    return {"active": False}


def migrate_user(source_root, target_root):
    source_root = managed(source_root).parent
    target_store = managed(target_root)
    source, target = source_root / "user", target_store.parent / "user"
    for path in (source, source / "user-dictionary", target):
        assert_plain_path(path)
    if not source.is_dir():
        raise ValueError("源目录缺少隔离用户资料")
    selected = [Path(name) for name in USER_CONFIG if (source / name).exists()]
    dictionary = source / "user-dictionary"
    if dictionary.is_dir():
        selected.extend(Path("user-dictionary") / path.name for path in sorted(dictionary.iterdir())
                        if USER_GENERATION.fullmatch(path.name))
    if not selected or len(selected) > len(USER_CONFIG) + 64:
        raise ValueError("没有可迁移的已发布用户资料，或文件数超限")

    with publish_lock(target_store):
        if target.exists():
            raise ValueError("目标用户目录已存在，保留现有资料")
        with tempfile.TemporaryDirectory(prefix=".user-migration-", dir=target.parent) as temporary:
            staged = Path(temporary) / "user"
            staged.mkdir()
            records = []
            for relative in selected:
                original = source / relative
                assert_plain_path(original)
                maximum = 8 * 1024 * 1024 if relative.parent.name else 1024 * 1024
                if not original.is_file() or original.stat().st_size > maximum:
                    raise ValueError("用户资料不是普通文件或超出大小上限")
                destination = staged / relative
                destination.parent.mkdir(exist_ok=True)
                shutil.copyfile(original, destination)
                with destination.open("r+b") as stream:
                    os.fsync(stream.fileno())
                    copied_sha = hashlib.file_digest(stream, "sha256").hexdigest()
                with original.open("rb") as stream:
                    source_sha = hashlib.file_digest(stream, "sha256").hexdigest()
                if copied_sha != source_sha:
                    raise ValueError("复制校验失败，保留源和原目标")
                records.append({"path": relative.as_posix(), "sha256": copied_sha,
                                "bytes": destination.stat().st_size})
            staged.rename(target)
    return {"state": "copied", "files": records}


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    commands = cli.add_subparsers(dest="command", required=True)
    seal_cmd = commands.add_parser("seal")
    seal_cmd.add_argument("release", type=Path)
    seal_cmd.add_argument("version")
    seal_cmd.add_argument("commit")
    dictionaries = commands.add_parser("stamp-dictionaries")
    dictionaries.add_argument("release", type=Path)
    dictionaries.add_argument("--base-payload")
    signing = commands.add_parser("signature-request")
    signing.add_argument("release", type=Path)
    migration = commands.add_parser("migrate-user")
    migration.add_argument("source_root", type=Path)
    migration.add_argument("target_root", type=Path)
    for action in ("init", "install", "status", "rollback", "uninstall", "recover"):
        cmd = commands.add_parser(action)
        cmd.add_argument("root", type=Path)
        if action == "install":
            cmd.add_argument("release", type=Path)
        if action == "recover":
            cmd.add_argument("writer_pid", type=int)
    settings = cli.parse_args()
    match settings.command:
        case "seal":
            output = seal(settings.release, settings.version, settings.commit)
        case "stamp-dictionaries":
            output = stamp_dictionaries(settings.release, settings.base_payload)
        case "signature-request":
            output = signature_request(settings.release)
        case "migrate-user":
            output = migrate_user(settings.source_root, settings.target_root)
        case "init":
            output = initialize(settings.root)
        case "install":
            output = install(settings.root, settings.release)
        case "status":
            output = status(settings.root)
        case "rollback":
            output = activate_previous(managed(settings.root), verifier=verify)
        case "uninstall":
            output = uninstall(settings.root)
        case "recover":
            output = recover_store(managed(settings.root), settings.writer_pid, verifier=verify)
    print(json.dumps(output, ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()

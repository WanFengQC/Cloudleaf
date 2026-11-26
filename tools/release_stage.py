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
ROLES = {"bin", "licenses", "provenance", "resources"}
COMMIT = re.compile(r"[a-f0-9]{40}")
VERSION = re.compile(r"[a-z0-9][a-z0-9.-]{0,47}")
SHA256 = re.compile(r"[a-f0-9]{64}")
USER_CONFIG = ("settings.json", "settings.json.bak", "input-settings.json", "input-settings.json.bak")
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
            or path.name.lower() in {"settings.json", "input-settings.json", "user-dictionary.tsv", "credentials.json"}):
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


def verify(release):
    assert_plain_path(release / MANIFEST)
    if (release / MANIFEST).stat().st_size > 2_000_000:
        raise ValueError("发行清单超限")
    manifest = read_json(release / MANIFEST)
    if (not isinstance(manifest, dict)
            or set(manifest) != {"schema_version", "scope", "version", "commit", "files"}
            or type(manifest["schema_version"]) is not int
            or manifest["schema_version"] != 1
            or manifest["scope"] != "isolated-rehearsal"
            or not VERSION.fullmatch(str(manifest["version"]))
            or not COMMIT.fullmatch(str(manifest["commit"]))
            or manifest["files"] != records(release)):
        raise ValueError("发行清单、文件身份或范围不匹配")
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
    manifest = {"schema_version": 1, "scope": "isolated-rehearsal",
                "version": version, "commit": commit, "files": records(release)}
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
    if not selected or len(selected) > 4 + 64:
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

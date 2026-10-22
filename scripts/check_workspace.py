"""按精确政策审计依赖、许可证与版本化资源，新增方向不得隐式放宽基线。"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def repository_file(root: Path, name: str) -> Path:
    """拒绝绝对路径、平台分隔符与逃逸，避免读取仓库外内容。"""
    relative = PurePosixPath(name)
    assert name and "\\" not in name and ":" not in name, "必须使用仓库内POSIX路径"
    assert not relative.is_absolute() and ".." not in relative.parts, "路径不可越界"
    path = (root / name).resolve()
    assert path.is_relative_to(root.resolve()) and path.is_file(), f"仓库文件不存在或越界：{name}"
    return path


def load_policy(root: Path) -> dict:
    paths = sorted((root / "config/audits").glob("*.json"))
    assert paths, "必须提供精确审计政策"
    combined = {"workspace_packages": set(), "external_packages": {}, "resources": {}}
    for path in paths:
        policy = json.loads(path.read_text(encoding="utf-8"))
        assert policy["schema_version"] == 1, "政策版本错误"
        for name in policy["workspace_packages"]:
            assert isinstance(name, str) and name, "crate名称无效"
            assert name not in combined["workspace_packages"], f"重复workspace声明：{name}"
            combined["workspace_packages"].add(name)
        for package in policy["external_packages"]:
            assert set(package) == {"name", "version", "license", "source"}, "依赖政策字段不精确"
            assert all(isinstance(value, str) and value for value in package.values()), "依赖字段必须非空"
            assert package["source"] == REGISTRY, "仅允许精确审核的registry包"
            key = package["name"], package["version"]
            assert key not in combined["external_packages"], f"重复依赖声明：{key}"
            combined["external_packages"][key] = package
        for resource in policy["resources"]:
            assert set(resource) == {"path", "source", "license", "license_file"}, "资源政策字段不精确"
            assert all(isinstance(value, str) and value for value in resource.values()), "资源字段必须非空"
            key = resource["path"]
            assert key not in combined["resources"], f"重复资源声明：{key}"
            repository_file(root, key)
            repository_file(root, resource["license_file"])
            combined["resources"][key] = resource
    return combined


def check_packages(metadata: dict, policy: dict, version: str) -> tuple[int, int]:
    members = set(metadata["workspace_members"])
    actual_members, actual_external = set(), {}
    for package in metadata["packages"]:
        if package["id"] in members:
            actual_members.add(package["name"])
            assert package["source"] is None, "workspace必须来自本地"
            assert package["license"] == "MIT", "workspace许可不一致"
            assert package["rust_version"] == version, "crate MSRV不一致"
            assert package["edition"] == "2024", "workspace edition不一致"
        else:
            key = package["name"], package["version"]
            assert key not in actual_external, "同名同版本不能来自多个来源"
            actual_external[key] = {field: package[field] for field in ("name", "version", "license", "source")}
    assert actual_members == policy["workspace_packages"], "实际workspace成员与政策不符"
    assert actual_external == policy["external_packages"], "实际外部依赖与审核版本/许可/来源不符"
    return len(members), len(actual_external)


def check_resources(root: Path, policy: dict) -> None:
    resources = set()
    for path in sorted((root / "data/manifests").glob("*.json")):
        manifest = json.loads(path.read_text(encoding="utf-8"))
        assert manifest["schema_version"] == 1, "资源清单版本错误"
        name = manifest["path"]
        assert name not in resources, "同一资源不能重复登记"
        resources.add(name)
        assert name in policy["resources"], f"未审核资源：{name}"
        expected = policy["resources"][name]
        assert {field: manifest[field] for field in expected} == expected, "资源来源或授权与政策不符"
        repository_file(root, manifest["license_file"])
        content = repository_file(root, name).read_bytes()
        text = content.decode("utf-8")
        assert not text.startswith("\ufeff"), "版本化资源不接受BOM"
        assert "\r" not in text, "版本化资源必须使用LF保持哈希稳定"
        assert hashlib.sha256(content).hexdigest() == manifest["sha256"].lower(), f"资源校验失败：{name}"
        lines = text.splitlines()
        resource_format = manifest.get("format", "tsv")
        if resource_format == "tsv":
            headers = manifest.get("header_lines", 2)
            assert type(headers) is int and headers >= 1 and len(lines) >= headers, "TSV表头无效"
            rows = lines[headers:]
            width = len(lines[headers - 1].split("\t"))
            assert all(line and len(line.split("\t")) == width for line in rows), "TSV字段数量不一致"
        elif resource_format == "jsonl":
            rows = lines
            assert all(line and isinstance(json.loads(line), dict) for line in rows), "JSONL必须逐行包含对象"
        else:
            raise AssertionError(f"未审核资源格式：{resource_format}")
        assert type(manifest["entry_count"]) is int and len(rows) == manifest["entry_count"], "资源条目数与清单不符"
        print(f"资源已验证：{name} ({len(rows)} 条)")
    assert resources == set(policy["resources"]), "资源清单与政策必须完整对应"


def check() -> None:
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))
    version = workspace["workspace"]["package"]["rust-version"]
    assert toolchain["toolchain"]["channel"] == version, "工具链必须与 MSRV 同步"
    assert (ROOT / "Cargo.lock").is_file(), "必须提交 Cargo.lock"
    assert (ROOT / "LICENSE").read_text(encoding="utf-8").startswith("MIT License")
    assert (ROOT / "THIRD_PARTY_NOTICES.md").is_file(), "必须登记依赖与资源审查"
    policy = load_policy(ROOT)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
        cwd=ROOT, encoding="utf-8",
    ))
    members, external = check_packages(metadata, policy, version)
    ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    assert f"toolchain install {version}" in ci, "CI 工具链不一致"

    check_resources(ROOT, policy)
    print(f"workspace 已验证：{members} 个crate，{external} 个已登记外部Rust依赖，MSRV {version}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    try:
        check()
    except (AssertionError, KeyError, OSError, TypeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"工程审计失败：{error}", file=sys.stderr)
        sys.exit(1)

"""检查可重复构建、零外部依赖基线、许可证与版本化测试资源。"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def check() -> None:
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))
    version = workspace["workspace"]["package"]["rust-version"]
    assert toolchain["toolchain"]["channel"] == version, "工具链必须与 MSRV 同步"
    assert (ROOT / "Cargo.lock").is_file(), "必须提交 Cargo.lock"
    assert (ROOT / "LICENSE").read_text(encoding="utf-8").startswith("MIT License")
    notices = (ROOT / "THIRD_PARTY_NOTICES.md").read_text(encoding="utf-8")
    assert "没有 crates.io" in notices, "需要明确当前依赖基线"
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
        cwd=ROOT, encoding="utf-8",
    ))
    members = set(metadata["workspace_members"])
    assert len(members) == 6, "新增 crate 需要同步审查与基线"
    for package in metadata["packages"]:
        assert package["id"] in members and package["source"] is None, "新增外部依赖必须审查并加入漏洞审计"
        assert package["license"] == "MIT", "workspace 许可不一致"
        assert package["rust_version"] == version, "crate MSRV 不一致"
    ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    assert f"toolchain install {version}" in ci, "CI 工具链不一致"

    manifests = sorted((ROOT / "data/manifests").glob("*.json"))
    resources = set()
    for path in manifests:
        manifest = json.loads(path.read_text(encoding="utf-8"))
        assert manifest["schema_version"] == 1, "资源清单版本错误"
        resource = (ROOT / manifest["path"]).resolve()
        assert resource.is_relative_to(ROOT), "资源路径必须在仓库内"
        assert manifest["path"] not in resources, "同一资源不能重复登记"
        resources.add(manifest["path"])
        content = resource.read_bytes()
        text = content.decode("utf-8")
        assert not text.startswith("\ufeff"), "版本化 TSV 不接受 BOM"
        assert hashlib.sha256(content).hexdigest() == manifest["sha256"].lower(), f"资源校验失败：{manifest['path']}"
        assert manifest["license"] == "MIT" and manifest["source"].startswith("cloudleaf:"), "当前基线只允许项目自编数据"
        license_path = (ROOT / manifest["license_file"]).resolve()
        assert license_path.is_relative_to(ROOT) and license_path.is_file(), "授权文件必须存在于仓库内"
        lines = text.splitlines()
        assert len(lines) - 2 == manifest["entry_count"], "资源条目数与清单不符"
        width = len(lines[1].split("\t"))
        assert all(len(line.split("\t")) == width for line in lines[2:]), "资源 TSV 字段数量不一致"
        print(f"资源已验证：{manifest['path']} ({manifest['entry_count']} 条)")
    assert resources == {"data/dictionaries/prototype.tsv", "tests/fixtures/input-v1.tsv"}, "词典与输入语料清单必须完整对应当前基线"
    print(f"workspace 已验证：{len(members)} 个 crate，0 个外部 Rust 依赖，MSRV {version}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    try:
        check()
    except (AssertionError, KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"工程审计失败：{error}", file=sys.stderr)
        sys.exit(1)

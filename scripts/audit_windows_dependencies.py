"""开发期核对实际注册包许可原件，并保存精确RustSec审计的来源证据。"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_licenses(root: Path, metadata: dict) -> list[dict]:
    members = set(metadata["workspace_members"])
    packages = [package for package in metadata["packages"] if package["id"] not in members]
    manifest = json.loads((root / "licenses/windows/manifest.json").read_text(encoding="utf-8"))
    expected_packages = {(package["name"], package["version"]) for package in packages}
    assert {(row["package"], row["version"]) for row in manifest} == expected_packages, "许可清单包集合与实际依赖不符"
    seen = set()
    result = []
    for package in packages:
        original = Path(package["manifest_path"]).parent
        original_files = {path.name: path for path in original.iterdir()
                          if path.is_file() and path.name.lower().startswith("license")}
        rows = [row for row in manifest if (row["package"], row["version"]) == (package["name"], package["version"])]
        matched = set()
        files = []
        for row in rows:
            name = row["file"]
            assert name not in seen, "许可副本重复登记"
            seen.add(name)
            assert "\\" not in name and ":" not in name, "许可必须使用仓库内POSIX路径"
            copied = (root / name).resolve()
            assert copied.is_relative_to(root.resolve()) and copied.is_file(), "许可副本不存在或越界"
            assert sha256(copied) == row["sha256"], "许可副本哈希与清单不符"
            prefix = f"{package['name']}-{package['version']}-"
            assert copied.name.startswith(prefix), "许可副本名称与包不符"
            source_name = copied.name.removeprefix(prefix)
            assert source_name in original_files and source_name not in matched, "许可原件缺失或重复"
            source = original_files[source_name]
            assert source.read_bytes() == copied.read_bytes(), "许可副本与Cargo原包字节不同"
            matched.add(source_name)
            files.append({"file": name, "sha256": row["sha256"], "bytes": copied.stat().st_size})
        assert matched == set(original_files) and matched, "必须完整保留实际包内的许可原件"
        result.append({"name": package["name"], "version": package["version"], "license": package["license"],
                       "source": package["source"], "repository": package.get("repository"),
                       "rust_version": package.get("rust_version"), "files": files})
    return result


def run() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audit-binary", default="cargo-audit")
    parser.add_argument("--db", type=Path, default=Path.home() / ".cargo/advisory-db")
    parser.add_argument("--output", type=Path, default=ROOT / ".local/windows-dependency-audit.json")
    args = parser.parse_args()
    binary = shutil.which(args.audit_binary)
    if not binary:
        raise ValueError("未找到cargo-audit；按README先安装固定0.22.2开发工具")
    version = subprocess.check_output([binary, "--version"], encoding="utf-8").strip()
    if version != "cargo-audit 0.22.2":
        raise ValueError("审计工具版本不符：必须固定0.22.2")
    # 许可证门禁与精确依赖政策同时执行，不能用一份自填清单代替实际metadata。
    from check_workspace import check_packages, load_policy
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=ROOT, encoding="utf-8"))
    check_packages(metadata, load_policy(ROOT), "1.90.0")
    lock_hash = sha256(ROOT / "Cargo.lock")
    licenses = verify_licenses(ROOT, metadata)
    command = [binary, "audit", "--file", str(ROOT / "Cargo.lock"), "--db", str(args.db), "--json", "-D", "warnings"]
    audited = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=False)
    report = json.loads(audited.stdout)
    database = report.get("database", {})
    assert database.get("last-commit") and database.get("last-updated"), "必须记录成功获取的数据库版本"
    assert sha256(ROOT / "Cargo.lock") == lock_hash, "审计期间锁文件发生变化"
    result = {"schema_version": 1, "measured_at_utc": datetime.now(timezone.utc).isoformat(),
              "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, encoding="utf-8").strip(),
              "lock_sha256": lock_hash, "tool_version": version, "tool_sha256": sha256(Path(binary)),
              "audit_command": [Path(binary).name, *command[1:]], "audit_exit_code": audited.returncode,
              "licenses": licenses, "rustsec": report,
              "scope": "开发期锁文件与已知漏洞审计，不替代原生unsafe、应用兼容或完整供应链评审；工具不进入输入路径"}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    if audited.returncode or report["vulnerabilities"]["found"] or report["warnings"]:
        raise ValueError("RustSec未通过；完整失败证据已保存")
    print(json.dumps({"verified": True, "package_count": len(licenses), "database": database,
                      "lock_sha256": lock_hash}, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    try:
        run()
    except (AssertionError, KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Windows依赖审计失败：{error}", file=sys.stderr)
        sys.exit(1)

"""只读核对离线运行时原件及限定补丁，不豁免RustSec检查。"""
from pathlib import Path, PurePosixPath
import hashlib
import json

SOURCE_LOCK_SHA256 = "bde52ab6cc3a79191be83490ea9ac92363050ecb37aaadad37552a0c0d3796ec"
VERSIONS = {name: "0.19.0" for name in (
    "gemm", "gemm-common", "gemm-f16", "gemm-f32", "gemm-f64", "gemm-c32", "gemm-c64"
)} | {"pulp": "0.22.3", "candle-core": "0.9.2"}


def original_bytes(name: str, relative: str, patched: bytes) -> bytes:
    """只允许依赖替换和ZIP9返回值API适配；反向重建后核原件SHA。"""
    if relative == "Cargo.toml":
        if name == "candle-core":
            new = b'[dependencies.zip]\nversion = "=9.2.1"'
            old = b'[dependencies.zip]\nversion = "7.2.0"'
        else:
            new = b'[dependencies.paste]\nversion = "=0.2.3"\npackage = "pastey"'
            old = b'[dependencies.paste]\nversion = "1"' if name == "pulp" else b'[dependencies.paste]\nversion = "1.0"'
        assert patched.count(new) == 1, "运行时依赖补丁内容不符"
        return patched.replace(new, old)
    assert name == "candle-core", "不允许修改其他依赖的Rust源码"
    if relative == "src/npy.rs":
        result = patched
        for variable in (b"reader", b"file"):
            new = b"let name = " + variable + b".name()?;\n                name.strip_suffix(NPY_SUFFIX).unwrap_or(&name).to_owned()"
            old = b"let name = " + variable + b".name();\n                name.strip_suffix(NPY_SUFFIX).unwrap_or(name).to_owned()"
            assert result.count(new) == 1, "NPY档案名称适配内容不符"
            result = result.replace(new, old)
        return result
    assert relative == "src/pickle.rs", "仅允许三处档案名称返回值适配"
    new = b".map(|f| f.map(|name| name.into_owned()))\n        .collect::<std::result::Result<Vec<String>, _>>()?;"
    old = b".map(|f| f.to_string())\n        .collect::<Vec<String>>();"
    assert patched.count(new) == 1, "PTH档案名称适配内容不符"
    return patched.replace(new, old)


def verify_offline_vendor(root: Path) -> dict[tuple[str, str], str]:
    root = root.resolve()
    raw = (root / "config/offline-runtime-source-patches.json").read_bytes()
    assert hashlib.sha256(raw).hexdigest() == SOURCE_LOCK_SHA256, "运行时原件锁SHA不符"
    records = json.loads(raw)
    assert len(records) == len(VERSIONS), "运行时路径依赖数量不符"
    assert {row["name"]: row["version"] for row in records} == VERSIONS, "运行时原件版本集合不符"
    reviewed = {}
    for row in records:
        name, version = row["name"], row["version"]
        folder = root / "vendor/offline" / name
        assert folder.resolve().is_relative_to(root), "运行时原件目录越界"
        assert row["url"] == f"https://crates.io/api/v1/crates/{name}/{version}/download", "原件URL不符"
        expected = {item["path"]: item for item in row["files"]}
        changes = {item["path"]: item["sha256"] for item in row["modified_files"]}
        allowed = {"Cargo.toml", "src/npy.rs", "src/pickle.rs"} if name == "candle-core" else {"Cargo.toml"}
        assert set(changes) == allowed, "补丁文件范围不符"
        actual = {path.relative_to(folder).as_posix() for path in folder.rglob("*") if path.is_file()}
        assert actual == set(expected), "原件有未登记的新增或缺失文件"
        for relative, identity in expected.items():
            posix = PurePosixPath(relative)
            assert not posix.is_absolute() and ".." not in posix.parts and "\\" not in relative, "原件文件名越界"
            path = folder / relative
            assert path.resolve().is_relative_to(folder.resolve()) and not path.is_symlink(), "原件不能由符号链接替换"
            data = path.read_bytes()
            if relative in changes:
                assert hashlib.sha256(data).hexdigest() == changes[relative], "补丁产物SHA不符"
                data = original_bytes(name, relative, data)
            assert len(data) == identity["bytes"] and hashlib.sha256(data).hexdigest() == identity["upstream_sha256"], "与固定上游原件不符"
        reviewed[(name, version)] = f"path+vendor/offline/{name}"
    return reviewed


def reviewed_local_source(root: Path, package: dict, reviewed: dict[tuple[str, str], str]) -> str:
    key = package["name"], package["version"]
    assert package["source"] is None and key in reviewed, "未批准的路径依赖"
    expected = root / "vendor/offline" / package["name"] / "Cargo.toml"
    assert Path(package["manifest_path"]).resolve() == expected.resolve(), "路径依赖实际位置不符"
    return reviewed[key]


if __name__ == "__main__":
    import sys
    root = Path(sys.argv[1])
    print(json.dumps({"verified": True, "packages": len(verify_offline_vendor(root)), "source_lock_sha256": SOURCE_LOCK_SHA256}))

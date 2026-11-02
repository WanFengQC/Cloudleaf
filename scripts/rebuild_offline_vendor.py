"""从固定Cargo原件重建离线运行时补丁；不删除或覆盖已有输出目录。"""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import io
import json
import tarfile
import urllib.request

SOURCE_LOCK_SHA256 = "bde52ab6cc3a79191be83490ea9ac92363050ecb37aaadad37552a0c0d3796ec"


def patch_file(name: str, relative: str, original: bytes) -> bytes:
    if relative == "Cargo.toml":
        if name == "candle-core":
            old = b'[dependencies.zip]\nversion = "7.2.0"'
            new = b'[dependencies.zip]\nversion = "=9.2.1"'
        else:
            old = b'[dependencies.paste]\nversion = "1"' if name == "pulp" else b'[dependencies.paste]\nversion = "1.0"'
            new = b'[dependencies.paste]\nversion = "=0.2.3"\npackage = "pastey"'
        assert original.count(old) == 1
        return original.replace(old, new)
    if name == "candle-core" and relative == "src/npy.rs":
        result = original
        for variable in (b"reader", b"file"):
            old = b"let name = " + variable + b".name();\n                name.strip_suffix(NPY_SUFFIX).unwrap_or(name).to_owned()"
            new = b"let name = " + variable + b".name()?;\n                name.strip_suffix(NPY_SUFFIX).unwrap_or(&name).to_owned()"
            assert result.count(old) == 1
            result = result.replace(old, new)
        return result
    if name == "candle-core" and relative == "src/pickle.rs":
        old = b".map(|f| f.to_string())\n        .collect::<Vec<String>>();"
        new = b".map(|f| f.map(|name| name.into_owned()))\n        .collect::<std::result::Result<Vec<String>, _>>()?;"
        assert original.count(old) == 1
        return original.replace(old, new)
    raise ValueError("清单请求了未定义补丁")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", type=Path, required=True)
    parser.add_argument("--source-cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--allow-download", action="store_true")
    args = parser.parse_args()
    lock = args.lock.read_bytes()
    assert hashlib.sha256(lock).hexdigest() == SOURCE_LOCK_SHA256, "固定源码锁SHA不符"
    records = json.loads(lock)
    args.output.mkdir(parents=True, exist_ok=False)
    for row in records:
        source = args.source_cache / f"{row['name']}-{row['version']}.crate"
        if source.exists():
            raw = source.read_bytes()
        else:
            if not args.allow_download:
                raise ValueError("缺少固定Cargo原件；未获准联网下载")
            with urllib.request.urlopen(row["url"], timeout=45) as response:
                raw = response.read(row["crate_bytes"] + 1)
            args.source_cache.mkdir(parents=True, exist_ok=True)
            assert len(raw) == row["crate_bytes"] and hashlib.sha256(raw).hexdigest() == row["crate_sha256"]
            with source.open("xb") as stream:
                stream.write(raw)
        assert len(raw) == row["crate_bytes"] and hashlib.sha256(raw).hexdigest() == row["crate_sha256"], "Cargo原件SHA不符"
        changes = {item["path"]: item["sha256"] for item in row["modified_files"]}
        folder = args.output / row["name"]
        folder.mkdir()
        with tarfile.open(fileobj=io.BytesIO(raw)) as archive:
            for entry in row["files"]:
                relative = entry["path"]
                posix = PurePosixPath(relative)
                assert not posix.is_absolute() and ".." not in posix.parts and "\\" not in relative
                member = archive.getmember(f"{row['name']}-{row['version']}/{relative}")
                assert member.isfile()
                data = archive.extractfile(member).read()
                assert len(data) == entry["bytes"] and hashlib.sha256(data).hexdigest() == entry["upstream_sha256"]
                if relative in changes:
                    data = patch_file(row["name"], relative, data)
                    assert hashlib.sha256(data).hexdigest() == changes[relative]
                target = folder / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                with target.open("xb") as stream:
                    stream.write(data)
    print(json.dumps({"packages": len(records), "source_lock_sha256": SOURCE_LOCK_SHA256, "download_enabled": args.allow_download}))


if __name__ == "__main__":
    main()

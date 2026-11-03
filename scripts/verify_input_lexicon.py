"""Round14固定中文词条包离线预检；凭据不替代签名或批准新数据。"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import platform
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from tools.data_pipeline.pipeline import VERSION, verify_release

SOURCE_COMMIT = "3f26f34e00c0775124f87e150a021f99fb52c567"
MANIFEST_SHA256 = "5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5"
RUNTIME_SHA256 = "03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431"
SOURCE_LOCK_SHA256 = "a7cbdd759ae2487ce238aea8a2d43eeef3248a1a124004def9ad35764e58bd57"
LICENSE_SHA256 = "28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5"
SOURCE_SHA256 = "fff8763673740aecbbe4c7f414445171ef4a70e55eac3ceee364e4f9c7cc766a"
SOURCE_LICENSES = (
    ("unihan-17.0.0.json", "Unicode-3.0.txt"),
    ("cccedict-20260604-mirror7019e1c.json", "CC-BY-SA-4.0.txt"),
)


def main() -> None:
    args = arguments()
    started = time.perf_counter()
    source_licenses = verify_source_licenses(ROOT)
    release = args.release.resolve(strict=True)
    if args.output.resolve().is_relative_to(release):
        raise ValueError("加载凭据必须放在release目录之外，保留原九文件包")
    manifest_path = release / "manifest.json"
    manifest_bytes = manifest_path.read_bytes()
    if sha256(manifest_bytes) != MANIFEST_SHA256:
        raise ValueError("不是本轮固定的已审词条包")
    manifest = verify_release(release)
    validate_scope(manifest)
    record = receipt(release, manifest, manifest_bytes, started)
    record["repository_source_license_sha256"] = source_licenses
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(record, output, ensure_ascii=False, indent=2)
        output.write("\n")
    print(json.dumps({"verified": True, "receipt": str(args.output.resolve()),
                      "runtime_sha256": RUNTIME_SHA256, "entries": manifest["entry_count"]},
                     ensure_ascii=False))


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError("输出已存在，拒绝覆盖")
    return args


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_source_licenses(root: Path) -> dict[str, str]:
    identities = {}
    for lock_name, license_name in SOURCE_LICENSES:
        lock = json.loads((root / "data/sources/locks" / lock_name).read_text(encoding="utf-8"))
        actual = sha256((root / "data/licenses" / license_name).read_bytes())
        if actual != lock["license_sha256"]:
            raise ValueError(f"来源许可原文字节与固定锁不一致：{license_name}")
        identities[license_name] = actual
    return identities


def validate_scope(manifest: dict) -> None:
    resources = manifest["resources"]
    source = manifest["source"]
    if (manifest["entry_count"] != 123544 or manifest["rejected_count"] != 1468
            or resources["runtime.tsv"]["sha256"] != RUNTIME_SHA256
            or resources["source-lock.json"]["sha256"] != SOURCE_LOCK_SHA256
            or resources["LICENSE.txt"]["sha256"] != LICENSE_SHA256
            or source["sha256"] != SOURCE_SHA256
            or source["source_id"] != "cccedict:mirror-20260604"
            or source["review_status"] != "approved"
            or source["license_id"] != "CC-BY-SA-4.0"):
        raise ValueError("固定来源、许可或运行时身份不匹配")


def receipt(release: Path, manifest: dict, manifest_bytes: bytes, started: float) -> dict:
    # verify_release已经逐文件核验原始字节、事实层、投影、索引和unknown语义。
    files = {name: {"path": str(release / name), **identity}
             for name, identity in manifest["resources"].items()}
    files["manifest.json"] = {"path": str(release / "manifest.json"),
                              "bytes": len(manifest_bytes), "sha256": MANIFEST_SHA256}
    modules = {str(path.relative_to(ROOT)).replace("\\", "/"): sha256(path.read_bytes())
               for path in sorted((ROOT / "tools/data_pipeline").glob("*.py"))}
    source = manifest["source"]
    return {"schema_version": 1, "kind": "cloudleaf-input-lexicon-load-receipt",
            "verified": True, "at_utc": datetime.now(timezone.utc).isoformat(),
            "command": list(sys.orig_argv), "seconds": time.perf_counter() - started,
            "environment": {"python": sys.version, "platform": platform.platform()},
            "data_pipeline_reference_commit": SOURCE_COMMIT, "verifier_version": VERSION,
            "tool_sha256": sha256(Path(__file__).read_bytes()), "verifier_files_sha256": modules,
            "release": str(release), "manifest_sha256": MANIFEST_SHA256,
            "runtime": files["runtime.tsv"], "files": files,
            "entry_count": manifest["entry_count"], "rejected_count": manifest["rejected_count"],
            "frequency": {"kind": "unknown", "missing": manifest["entry_count"],
                          "synthetic_values_created": 0, "observed_counts_available": False},
            "source": {"id": source["source_id"], "version": source["version"],
                       "sha256": SOURCE_SHA256, "source_lock_sha256": SOURCE_LOCK_SHA256,
                       "license_id": source["license_id"], "license_sha256": LICENSE_SHA256,
                       "approved_scope": source["approved_scope"],
                       "attribution": source["attribution"], "redistribution": source["redistribution"]},
            "boundary": {"fixed_word_pronunciation_gloss_scope_only": True,
                         "production_frequency_approved": False,
                         "independent_language_quality_accepted": False,
                         "cryptographic_signature_verified": False,
                         "approves_new_data": False, "network_downloads": False,
                         "note": "只记录固定已审范围包的完整离线校验；Rust装载仍核runtime SHA，不把凭据当信任根。"}}


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"输入词典预检失败：{error}", file=sys.stderr)
        raise SystemExit(1)

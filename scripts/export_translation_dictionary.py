"""将已固定schema2原始词头/英语释义投影为研究索引，不替代CL-009生产验收。"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil

RESOURCES = {"LICENSE.txt", "NOTICE.txt", "index.json", "quality.json", "records.jsonl", "rejected.jsonl", "runtime.tsv", "source-lock.json"}


def _digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("JSON字段重复")
        result[key] = value
    return result


def _constant(_):
    raise ValueError("JSON不接受非有限数值")


def _json(raw: bytes):
    return json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_object, parse_constant=_constant)


def _hash(value):
    return isinstance(value, str) and len(value) == 64 and all(letter in "0123456789abcdefABCDEF" for letter in value)


def _text(value, maximum):
    return isinstance(value, str) and bool(value.strip()) and len(value.encode("utf-8")) <= maximum and "\0" not in value


def _package(package: Path, expected: str):
    manifest_path = package / "manifest.json"
    if not _hash(expected) or not manifest_path.is_file() or _digest(manifest_path) != expected.lower():
        raise ValueError("固定package manifest SHA不符或缺失")
    manifest = _json(manifest_path.read_bytes())
    if (not isinstance(manifest, dict) or type(manifest.get("schema_version")) is not int
            or manifest["schema_version"] != 2 or manifest.get("runtime_format") != "cloudleaf-lexicon-v2"
            or not isinstance(manifest.get("resources"), dict) or set(manifest["resources"]) != RESOURCES
            or type(manifest.get("entry_count")) is not int or not 1 <= manifest["entry_count"] <= 1_000_000):
        raise ValueError("固定schema2研究投影的文件集合或版本不符")
    for name, descriptor in manifest["resources"].items():
        path = package / name
        if (not isinstance(descriptor, dict) or set(descriptor) != {"bytes", "sha256"}
                or type(descriptor["bytes"]) is not int or descriptor["bytes"] < 0
                or not _hash(descriptor["sha256"]) or not path.is_file()
                or path.stat().st_size != descriptor["bytes"] or _digest(path) != descriptor["sha256"].lower()):
            raise ValueError(f"固定资源身份不符:{name}")
    lock = _json((package / "source-lock.json").read_bytes())
    if (not isinstance(lock, dict) or lock != manifest.get("source") or lock.get("review_status") != "approved"
            or lock.get("format") != "cedict-zip-v1" or not _hash(lock.get("sha256"))
            or not _text(lock.get("source_id"), 128) or not _text(lock.get("version"), 128)
            or not _text(lock.get("license_id"), 128) or "gloss" not in str(lock.get("approved_scope", "")).lower()
            or lock.get("license_sha256") != manifest["resources"]["LICENSE.txt"]["sha256"]
            or manifest.get("source_lock_sha256") != manifest["resources"]["source-lock.json"]["sha256"]):
        raise ValueError("来源锁未覆盖固定原始gloss或许可身份不符")
    return manifest, lock


def export_dictionary(package: Path, manifest_sha256: str, output: Path) -> dict:
    package, output = Path(package), Path(output)
    manifest, lock = _package(package, manifest_sha256)
    entries, ids = [], set()
    glosses = 0
    with (package / "records.jsonl").open("rb") as stream:
        for number, raw in enumerate(stream, 1):
            row = _json(raw)
            if (not isinstance(row, dict) or type(row.get("schema_version")) is not int or row["schema_version"] != 2
                    or not _text(row.get("id"), 128) or row["id"] in ids
                    or not _text(row.get("source_record_id"), 128) or not _text(row.get("text"), 1024)
                    or row.get("source") != lock["source_id"] or row.get("source_version") != lock["version"]
                    or row.get("license_id") != lock["license_id"] or not isinstance(row.get("raw_senses"), list)
                    or not 1 <= len(row["raw_senses"]) <= 64 or any(not _text(value, 4096) for value in row["raw_senses"])):
                raise ValueError(f"原始词头/gloss行无效:{number}")
            ids.add(row["id"])
            entries.append({"id": row["id"], "source_id": row["source"], "source_record_id": row["source_record_id"],
                            "headword": row["text"], "raw_glosses": row["raw_senses"], "status": "research_only"})
            glosses += len(row["raw_senses"])
    if len(entries) != manifest["entry_count"]:
        raise ValueError("记录分母与固定manifest不符")
    source = {"id": lock["source_id"], "version": lock["version"], "artifact_sha256": lock["sha256"],
              "license_id": lock["license_id"], "license_file": "LICENSE.txt", "license_sha256": _digest(package / "LICENSE.txt"),
              "notice_file": "NOTICE.txt", "notice_sha256": _digest(package / "NOTICE.txt"), "source_lock_file": "SOURCE.json",
              "source_lock_sha256": _digest(package / "source-lock.json"), "review_status": "approved_bounded_scope", "scope": ["raw_gloss"]}
    catalog = {"format": "cloudleaf-translation-dictionary-v1", "schema_version": 1,
               "version": lock["source_id"] + ":" + manifest["resources"]["records.jsonl"]["sha256"][:24],
               "sources": [source], "entries": entries, "mappings": []}
    encoded = (json.dumps(catalog, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
    if len(encoded) > 128 * 1024 * 1024:
        raise ValueError("研究索引超出reader字节预算")
    output.mkdir(parents=True, exist_ok=False)
    for old, new in [("LICENSE.txt", "LICENSE.txt"), ("NOTICE.txt", "NOTICE.txt"), ("source-lock.json", "SOURCE.json")]:
        shutil.copyfile(package / old, output / new)
    (output / "catalog.json").write_bytes(encoded)
    (output / "MODIFICATIONS.txt").write_text(
        "Cloudleaf Round 11 research projection: retain original entry IDs, headwords and exact English raw gloss strings; "
        "build forward/reverse lookup only. No POS, sense alignment, phrase equivalence, frequency or production approval generated. "
        "Original attribution and license remain in NOTICE.txt, LICENSE.txt and SOURCE.json.\n", encoding="utf-8", newline="\n")
    report = {"format": "cloudleaf-translation-dictionary-export-v1", "source_package": str(package.resolve()),
              "source_package_manifest_sha256": manifest_sha256.lower(), "source_records_sha256": manifest["resources"]["records.jsonl"]["sha256"],
              "source_lock_sha256": source["source_lock_sha256"], "source_artifact_sha256": source["artifact_sha256"],
              "source_license_id": source["license_id"], "entries": len(entries), "raw_glosses": glosses,
              "translation_mappings_created": 0, "all_entry_quality": "research_only", "production_ready": False,
              "human_identity_or_rights_proven": False, "language_quality_evaluated": False,
              "tool_sha256": _digest(Path(__file__)), "catalog_sha256": hashlib.sha256(encoded).hexdigest(),
              "catalog_bytes": len(encoded), "output_files": {path.name: {"bytes": path.stat().st_size, "sha256": _digest(path)}
                                                               for path in sorted(output.iterdir()) if path.is_file()},
              "scope": "Research projection and fixed file identity checks; does not replace CL-009 verify_release or approve semantic content."}
    (output / "export-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = export_dictionary(args.package, args.manifest_sha256, args.output)
    print(json.dumps({key: report[key] for key in ["entries", "raw_glosses", "catalog_sha256", "production_ready"]}))


if __name__ == "__main__":
    main()

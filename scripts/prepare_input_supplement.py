"""从已固定的批准原件投影独立研发词典层，保留原包及unknown。"""
from pathlib import Path
from collections import defaultdict
import argparse
import hashlib
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from tools.data_pipeline.cedict_source import read_archive, parse_reading, source_key

BASELINE_SHA = "03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431"
MANIFEST_SHA = "5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-release", type=Path, required=True)
    parser.add_argument("--source-archive", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--model", type=Path, action="append", default=[])
    args = parser.parse_args()
    report = prepare(args.baseline_release, args.source_archive, args.output_dir, args.model)
    print(json.dumps(report, ensure_ascii=False, indent=2))


def digest(data):
    return hashlib.sha256(data).hexdigest()


def traditional_rows(records, source_lines, source_id, existing):
    grouped = defaultdict(list)
    for record in records:
        text = record["traditional"]
        pinyin = " ".join(record["pinyin"])
        if len(text) != 1 or len(record["pinyin"]) != 1 or text == record["text"] or (text, pinyin) in existing:
            continue
        if not record["evidence"]:
            raise ValueError("缺少批准记录的原件证据")
        for evidence in record["evidence"]:
            raw = source_lines[evidence["line"] - 1]
            match = re.fullmatch(r"([^ ]+) ([^ ]+) \[([^]]+)\] /(.*)/", raw)
            if (raw != evidence["raw"] or evidence["member"] != "cedict_ts.u8" or not match
                    or match.group(1) != text or match.group(2) != record["simplified"]
                    or parse_reading(match.group(3)) != record["pinyin"]
                    or source_key(*match.groups()[:3]) != record["source_record_id"]):
                raise ValueError("批准记录与固定原件不一致")
        grouped[(text, pinyin)].append(record)
    rows, lineage = [], []
    for (text, pinyin), provenance in sorted(grouped.items()):
        provenance.sort(key=lambda r: (r["source_record_id"], r["id"]))
        payload = json.dumps([source_id, text, pinyin], ensure_ascii=False, separators=(",", ":")).encode()
        ident = "entry:trad-r17:" + digest(payload)[:32]
        fields = [ident, pinyin, text, "", "unknown", "", "cccedict:mirror-20260604:traditional-r17",
                  provenance[0]["source_record_id"], "CC-BY-SA-4.0"]
        rows.append("\t".join(fields) + "\n")
        lineage.append({"id": ident, "text": text, "pinyin": pinyin, "frequency": None,
            "frequency_kind": "unknown", "source": fields[6], "license_id": fields[-1],
            "original_record_ids": [r["id"] for r in provenance],
            "source_record_ids": [r["source_record_id"] for r in provenance],
            "source_evidence": [e for r in provenance for e in r["evidence"]],
            "usage": "research_only", "production_approved": False})
    return rows, lineage


def prepare(baseline, source_archive, output, models):
    manifest_bytes = (baseline / "manifest.json").read_bytes()
    runtime = (baseline / "runtime.tsv").read_bytes()
    if digest(manifest_bytes) != MANIFEST_SHA or digest(runtime) != BASELINE_SHA:
        raise ValueError("固定基线身份不匹配")
    manifest = json.loads(manifest_bytes)
    record_bytes = (baseline / "records.jsonl").read_bytes()
    lock = json.loads((ROOT / "data/sources/locks/cccedict-20260604-mirror7019e1c.json").read_text(encoding="utf-8"))
    if digest(record_bytes) != manifest["resources"]["records.jsonl"]["sha256"] or digest(source_archive.read_bytes()) != lock["sha256"]:
        raise ValueError("批准记录或原始归档身份不匹配")
    original, _ = read_archive(source_archive, lock)
    records = [json.loads(line) for line in record_bytes.decode("utf-8").splitlines()]
    lines = runtime.decode("utf-8").splitlines()
    existing = {(f[2], f[1]) for f in (line.split("\t") for line in lines[2:])}
    rows, lineage = traditional_rows(records, original.splitlines(), lock["source_id"], existing)
    body = "".join(rows).encode("utf-8")
    combined = runtime + body
    combined_sha = digest(combined)
    model_outputs = []
    for model in models:
        raw = model.read_bytes()
        text = raw.decode("utf-8")
        fields = text.splitlines()
        if (len(raw) > 16 * 1024 * 1024 or len(fields) < 11
                or fields[0] != "# cloudleaf-language-v1" or fields[1] != "lexicon_sha256\t" + BASELINE_SHA
                or fields[6] not in ["usage\tresearch_only", "usage\tmechanical"]):
            raise ValueError("模型必须是绑定原基线的独立研发快照")
        # 只改变词典绑定；计数、训练来源和评分种类原字节保留，实际Rust reader再编译新缓存。
        rebound = text.replace(fields[1] + "\n", "lexicon_sha256\t" + combined_sha + "\n", 1).encode("utf-8")
        model_outputs.append((model, raw, rebound))
    output.mkdir(parents=True, exist_ok=False)
    (output / "layer.tsv").write_bytes(("\n".join(lines[:2]) + "\n").encode() + body)
    (output / "combined-runtime.tsv").write_bytes(combined)
    (output / "lineage.jsonl").write_text("".join(json.dumps(r, ensure_ascii=False, sort_keys=True) + "\n" for r in lineage), encoding="utf-8", newline="\n")
    bindings = []
    for index, (source, raw, rebound) in enumerate(model_outputs):
        name = f"model-{index}.tsv"
        (output / name).write_bytes(rebound)
        bindings.append({"source_path": str(source), "source_sha256": digest(raw), "output": name,
                         "sha256": digest(rebound), "counts_unchanged": True})
    for name in ["LICENSE.txt", "NOTICE.txt"]:
        (output / name).write_bytes((baseline / name).read_bytes())
    (output / "MODIFICATIONS.txt").write_text("Research-only traditional single-character layer projected from accepted fixed CC-CEDICT source records. Baseline rows unchanged; no frequency or pronunciation invented. Model dictionary binding changed where explicitly requested. Not approved for production.\n", encoding="utf-8", newline="\n")
    report = {"schema_version": 1, "usage": "research_only", "production_approved": False,
        "baseline_sha256": BASELINE_SHA, "source_zip_sha256": lock["sha256"], "baseline_entries": manifest["entry_count"],
        "layer_entries": len(rows), "layer_characters": len({r["text"] for r in lineage}),
        "combined_sha256": combined_sha, "baseline_exact_byte_prefix_preserved": combined.startswith(runtime),
        "model_bindings": bindings,
        "files": {p.name: {"bytes": p.stat().st_size, "sha256": digest(p.read_bytes())} for p in sorted(output.iterdir()) if p.is_file()}}
    (output / "manifest.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    return report


if __name__ == "__main__":
    main()

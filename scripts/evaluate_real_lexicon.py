"""校验证据后评估真实词条的搜索一致性和资源；没有独立gold，不计算语言准确率。"""
from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys

from benchmark import environment, source_sha256
from evaluate_search import measure

ROOT = Path(__file__).resolve().parents[1]
CASES = ROOT / "tests/search-resources-v1/cases.tsv"
PROTOCOL = ROOT / "docs/cl008-real-resource-protocol.md"
CONFIGURATIONS = {
    "reference": ("reference", 64, 16384, 200000, 50000),
    "exact": ("exact", 64, 16384, 200000, 50000),
    "beam1": ("beam", 1, 16384, 200000, 50000),
    "beam8": ("beam", 8, 16384, 200000, 50000),
    "beam64": ("beam", 64, 16384, 200000, 50000),
    "exact-edges128": ("exact", 64, 128, 200000, 50000),
    "exact-expansions500": ("exact", 64, 16384, 500, 50000),
    "exact-resident128": ("exact", 64, 16384, 200000, 128),
}
HEADER = ["id", "pinyin", "text", "frequency", "frequency_kind", "annotation", "source", "source_record_id", "license_id"]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def checked_file(item: dict, folder: Path, label: str) -> Path:
    path = (folder / item["path"]).resolve()
    if not path.is_file() or sha256(path) != item["sha256"]:
        raise ValueError(f"{label}文件缺失或SHA-256不符")
    return path


def validate_evidence(path: Path) -> tuple[Path, dict, dict]:
    evidence = json.loads(path.read_text(encoding="utf-8"))
    if evidence.get("schema_version") != 1 or evidence.get("real_data") is not True or not evidence.get("dataset_id"):
        raise ValueError("真实数据证据版本或标记无效")
    runtime = checked_file(evidence["runtime"], path.parent, "runtime")
    checked_file(evidence["build_report"], path.parent, "构建报告")
    approved = {}
    for source in evidence["sources"]:
        key = source["id"]
        if (key in approved or source.get("review_status") != "approved"
                or source.get("synthetic") is not False or not source.get("version")
                or not source.get("url", "").startswith("https://") or not source.get("license_id")):
            raise ValueError("来源缺少批准/版本/许可，或重复/合成")
        checked_file(source["license"], path.parent, "许可证")
        checked_file(source["audit"], path.parent, "来源审核")
        approved[key] = source
    if not approved:
        raise ValueError("来源清单为空")
    if runtime.stat().st_size > 128 * 1024 * 1024:
        raise ValueError("runtime超出本轮载入预算")
    # 同时核对每条运行时来源和许可，不用总文件哈希替代逐记录证据关联。
    with runtime.open(encoding="utf-8", newline="") as stream:
        if stream.readline() != "# cloudleaf-lexicon-v2\n":
            raise ValueError("仅接受严格LF v2")
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != HEADER:
            raise ValueError("v2表头无效")
        counts = {"entries": 0, "unknown_frequencies": 0, "multi_character_entries": 0}
        source_counts = {key: 0 for key in approved}
        ids = set()
        for row in reader:
            source = approved.get(row.get("source"))
            if (None in row or None in row.values() or not source or not row["source_record_id"]
                    or row["license_id"] != source["license_id"] or row["id"] in ids):
                raise ValueError("记录缺字段、来源/许可未登记或ID重复")
            if row["frequency_kind"] == "manual":
                raise ValueError("人工频率不是本轮真实观测数据")
            if row["frequency_kind"] == "unknown":
                if row["frequency"]:
                    raise ValueError("unknown频率不能补值")
                counts["unknown_frequencies"] += 1
            elif row["frequency_kind"] != "count" or not row["frequency"].isdigit():
                raise ValueError("词频类型或值无效")
            ids.add(row["id"])
            source_counts[row["source"]] += 1
            counts["entries"] += 1
            counts["multi_character_entries"] += len(row["text"]) > 1
    if counts["entries"] == 0 or counts["entries"] > 1_000_000:
        raise ValueError("真实词条数量不在载入预算内")
    counts["source_counts"] = source_counts
    return runtime, evidence, counts


def validate_cases(path: Path = CASES) -> list[dict]:
    raw = path.read_bytes().decode("utf-8")
    first, _, body = raw.partition("\n")
    if first != "# cloudleaf-search-resource-cases-v1" or "\r" in raw:
        raise ValueError("探针版本或编码无效")
    reader = csv.DictReader(io.StringIO(body), delimiter="\t")
    if reader.fieldnames != ["id", "family", "input", "kind"]:
        raise ValueError("探针表头无效，禁止附加gold")
    rows = list(reader)
    if not rows or len({row["id"] for row in rows}) != len(rows):
        raise ValueError("探针为空或ID重复")
    if any(None in row or None in row.values() or not row["id"] or not row["family"] or not row["kind"] for row in rows):
        raise ValueError("探针字段无效")
    return rows


def build_host() -> Path:
    command = ["cargo", "build", "--release", "--locked", "--offline", "-p", "cloudleaf-search",
               "--example", "resource_probe", "--message-format=json"]
    build = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, encoding="utf-8")
    artifacts = []
    for line in build.stdout.splitlines():
        row = json.loads(line)
        if row.get("reason") == "compiler-message":
            print(row["message"].get("rendered", ""), file=sys.stderr, end="")
        if (row.get("reason") == "compiler-artifact" and row.get("target", {}).get("name") == "resource_probe"
                and row.get("executable")):
            artifacts.append(Path(row["executable"]))
    if build.returncode or len(artifacts) != 1 or not artifacts[0].is_file():
        raise RuntimeError(f"真实词条实验宿主构建失败：{build.stderr}")
    return artifacts[0]


def texts(case: dict) -> list[str]:
    return [candidate["text"] for candidate in case["candidates"]]


def delta(before: list[str], after: list[str]) -> dict:
    old = {value: index + 1 for index, value in enumerate(before)}
    new = {value: index + 1 for index, value in enumerate(after)}
    return {"added": [value for value in after if value not in old],
            "lost": [value for value in before if value not in new],
            "rank_changes": [{"text": value, "before": old[value], "after": new[value]} for value in after
                             if value in old and old[value] != new[value]], "order_changed": before != after}


def compare(runs: dict, cases: list[dict]) -> tuple[list[dict], list[str]]:
    expected = {case["id"] for case in cases}
    indexed = {}
    for label, run in runs.items():
        if len(run["cases"]) != len(expected) or {case["id"] for case in run["cases"]} != expected:
            raise ValueError("搜索策略丢失案例或重复ID")
        indexed[label] = {case["id"]: case for case in run["cases"]}
    output, failures = [], []
    for case in cases:
        key = case["id"]
        reference = indexed["reference"][key]
        exact = indexed["exact"][key]
        reference_complete = reference["status"] == "ok" and reference["guaranteed"]
        exact_complete = exact["status"] == "ok" and exact["guaranteed"]
        different = any(reference.get(field) != exact.get(field) for field in ("status", "error_kind", "candidates", "waiting", "fallback_preserved"))
        if reference_complete and exact_complete and different:
            failures.append(key)
        methods = {}
        for label in runs:
            value = indexed[label][key]
            statistics = value.get("statistics", {})
            methods[label] = {"vs_exact": delta(texts(exact), texts(value)),
                              "vs_reference": delta(texts(reference), texts(value)),
                              "exact_comparison_valid": exact_complete,
                              "reference_comparison_valid": reference_complete,
                              "observable_pruning": {"budget": statistics.get("budget"), "beam_pruned": statistics.get("beam_pruned", 0)},
                              "candidate_availability": bool(value["candidates"]), "status": value["status"]}
        output.append({**case, "reference_complete": reference_complete, "exact_complete": exact_complete,
                       "exact_reference_equal": not different if reference_complete and exact_complete else None,
                       "whole_entry_vs_exact": {"comparison_valid": exact.get("whole_entry_complete", False) and exact_complete,
                                                "whole_entry_complete": exact.get("whole_entry_complete", False),
                                                "exact_complete": exact_complete,
                                                "observed_delta": delta(exact.get("whole_entry_candidates", []), texts(exact))},
                       "methods": methods})
    return output, failures


def run() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--iterations", type=int, default=30)
    parser.add_argument("--warmup", type=int, default=5)
    parser.add_argument("--output", type=Path, default=ROOT / ".local/real-lexicon-search.json")
    args = parser.parse_args()
    if not 1 <= args.iterations <= 10000 or not 0 <= args.warmup <= 1000:
        parser.error("测量次数超限")
    runtime, evidence, counts = validate_evidence(args.evidence)
    cases = validate_cases()
    evidence_hash, runtime_hash, before = sha256(args.evidence), sha256(runtime), source_sha256()
    binary = build_host()
    runs = {}
    for label, (method, width, edges, expansions, resident) in CONFIGURATIONS.items():
        command = [str(binary), str(runtime), str(CASES), method, str(width), str(args.iterations),
                   str(args.warmup), str(edges), str(expansions), str(resident), "5"]
        result, peak, memory_method = measure(command)
        result.update(command=command, peak_process_memory_bytes=peak, memory_method=memory_method,
                      configuration={"method": method, "beam_width": width, "max_edges": edges,
                                     "max_expansions": expansions, "max_resident_paths": resident,
                                     "max_input_bytes": 512, "max_letters": 256, "output_k": 5, "ranker": "Structural"})
        runs[label] = result
    comparisons, failures = compare(runs, cases)
    validate_evidence(args.evidence)
    if source_sha256() != before or sha256(args.evidence) != evidence_hash or sha256(runtime) != runtime_hash:
        raise RuntimeError("构建/测量期间源码或数据证据发生变化")
    summaries = {}
    for label, result in runs.items():
        rows = result["cases"]
        summaries[label] = {"total": len(rows), "candidate_nonempty": sum(bool(row["candidates"]) for row in rows),
                            "rejected": sum(row["status"] == "rejected" for row in rows),
                            "budget_triggered": sum(row.get("statistics", {}).get("budget") is not None for row in rows),
                            "beam_pruned_cases": sum(row.get("statistics", {}).get("beam_pruned", 0) > 0 for row in rows),
                            "max_case_p95_ns": max(row["p95_ns"] for row in rows),
                            "max_case_p99_ns": max(row["p99_ns"] for row in rows),
                            "peak_process_memory_bytes": result["peak_process_memory_bytes"]}
    report = {"schema_version": 1, "experiment": "cloudleaf-real-lexicon-resource-v1", "language_accuracy_valid": False,
              "top1": None, "top5": None, "mrr": None, "frequency_quality": None, "language_model_quality": None,
              "measured_at_utc": datetime.now(timezone.utc).isoformat(), "environment": environment(),
              "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, encoding="utf-8").strip(),
              "rustc": subprocess.check_output(["rustc", "--version"], encoding="utf-8").strip(),
              "source_fingerprint_schema": "cloudleaf-source-v2", "source_sha256": before,
              "binary_sha256": sha256(binary), "cases_sha256": sha256(CASES), "protocol_sha256": sha256(PROTOCOL),
              "evidence_sha256": evidence_hash, "evidence": evidence, "lexicon_counts": counts,
              "summaries": summaries, "comparisons": comparisons, "exact_reference_failures": failures, "runs": runs,
              "limitations": "真实词条资源探针，无独立gold/授权train/dev/heldout，不计算准确率；Structural禁用频率和模型；进程峰值包含词库/索引/候选/宿主/样本；背景负载未控制，非TSF或产品性能。"}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "summaries": summaries, "exact_reference_failures": failures}, ensure_ascii=False))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    run()

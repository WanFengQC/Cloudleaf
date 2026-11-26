"""只读核对固定搜索实验，展开逐候选差异和资源分组；不生成语言正确答案。"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sys

from evaluate_real_lexicon import CASES, PROTOCOL, CONFIGURATIONS, checked_file, validate_cases, validate_evidence
from evaluate_search import DATA, read_table

ROOT = Path(__file__).resolve().parents[1]
CANDIDATE_FIELDS = ("text", "score", "ids", "features", "records")
RESULT_FIELDS = ("status", "error_kind", "waiting", "fallback_preserved", "finite_pool_only")
SHA = re.compile(r"[0-9a-f]{64}\Z")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def quantiles(samples: list[int]) -> dict:
    if not samples:
        return {"samples": 0, "p50_ns": None, "p95_ns": None, "p99_ns": None, "max_ns": None}
    ordered = sorted(samples)
    return {"samples": len(samples), **{f"p{p}_ns": ordered[(len(ordered) * p + 99) // 100 - 1]
                                        for p in (50, 95, 99)}, "max_ns": ordered[-1]}


def complete(row: dict) -> bool:
    statistics = row.get("statistics", {})
    return (row.get("status") == "ok" and row.get("guaranteed") is True
            and statistics.get("budget") is None and statistics.get("beam_pruned") == 0
            and row.get("whole_entry_complete", True) is True)


def incomplete_reasons(row: dict) -> list[str]:
    reasons = []
    if row.get("status") != "ok":
        reasons.append("rejected")
    if row.get("guaranteed") is not True:
        reasons.append("not_guaranteed")
    stats = row.get("statistics", {})
    if stats.get("budget") is not None:
        reasons.append("budget:" + str(stats["budget"]))
    if stats.get("beam_pruned", 0):
        reasons.append("beam_pruned")
    if row.get("whole_entry_complete", True) is not True:
        reasons.append("graph_incomplete")
    return reasons


def compare_rows(reference: dict, candidate: dict, records_required: bool) -> dict:
    before, after = reference.get("candidates", []), candidate.get("candidates", [])
    ranks = []
    fields = CANDIDATE_FIELDS if records_required else CANDIDATE_FIELDS[:-1]
    for index in range(max(len(before), len(after))):
        left = before[index] if index < len(before) else None
        right = after[index] if index < len(after) else None
        equal_fields = {field: left is not None and right is not None and left.get(field) == right.get(field)
                        for field in fields}
        item = {"rank": index + 1, "equal": left == right, "equal_fields": equal_fields}
        if left != right:
            item.update(reference=left, candidate=right)
        ranks.append(item)
    status_equal = {field: reference.get(field) == candidate.get(field) for field in RESULT_FIELDS}
    observed_equal = before == after and all(status_equal.values())
    valid = complete(reference) and complete(candidate)
    old = {row["text"]: rank + 1 for rank, row in enumerate(before)}
    new = {row["text"]: rank + 1 for rank, row in enumerate(after)}
    return {"reference_complete": complete(reference), "candidate_complete": complete(candidate),
            "full_equivalence": observed_equal if valid else None, "observed_equal": observed_equal,
            "reference_incomplete_reasons": incomplete_reasons(reference),
            "candidate_incomplete_reasons": incomplete_reasons(candidate), "status_equal": status_equal,
            "rank_checks": ranks,
            "lost": [row["text"] for row in before if row["text"] not in new],
            "added": [row["text"] for row in after if row["text"] not in old],
            "rank_changes": [{"text": row["text"], "before": old[row["text"]], "after": new[row["text"]]}
                             for row in after if row["text"] in old and old[row["text"]] != new[row["text"]]]}


def index_cases(rows: list[dict]) -> dict:
    if not rows or any(not isinstance(row.get("id"), str) or not row["id"] for row in rows):
        raise ValueError("案例缺少ID")
    indexed = {row["id"]: row for row in rows}
    if len(rows) != len(indexed):
        raise ValueError("案例ID重复")
    return indexed


def validate_samples(row: dict, iterations: int, real: bool) -> None:
    samples = row.get("samples_ns")
    if (not isinstance(samples, list) or len(samples) != iterations
            or any(type(value) is not int or value < 0 for value in samples)):
        raise ValueError("原始计时缺失、类型错误或样本数不符")
    values = quantiles(samples)
    if any(row.get(field) != values[field] for field in ("p50_ns", "p95_ns", "p99_ns", "max_ns")):
        raise ValueError("分位数与原始计时不符")
    if row.get("status") not in ("ok", "rejected"):
        raise ValueError("案例状态无效")
    if row["status"] == "ok":
        stats = row.get("statistics")
        if (not isinstance(stats, dict) or any(type(stats.get(field)) is not int or stats[field] < 0
                for field in ("graph_edges", "expansions", "resident_high_water", "beam_pruned"))
                or "budget" not in stats or type(row.get("guaranteed")) is not bool
                or type(row.get("waiting")) is not bool or type(row.get("fallback_preserved")) is not bool):
            raise ValueError("搜索状态或工作量缺失")
    candidates = row.get("candidates", [])
    if not isinstance(candidates, list) or len({item["text"] for item in candidates}) != len(candidates):
        raise ValueError("候选不是唯一文本序列")
    for item in candidates:
        fields = CANDIDATE_FIELDS if real else CANDIDATE_FIELDS[:-1]
        if (any(field not in item for field in fields) or type(item["score"]) is not int
                or not isinstance(item["ids"], list) or not isinstance(item["features"], dict)
                or (real and not isinstance(item["records"], list))):
            raise ValueError("候选评分、路径、特征或来源缺失")


def validate_report(report: dict, real: bool) -> dict:
    experiment = "cloudleaf-real-lexicon-resource-v1" if real else "cloudleaf-search-mechanics-v1"
    if (report.get("schema_version") != 1 or report.get("experiment") != experiment
            or report.get("language_accuracy_valid") is not False):
        raise ValueError("实验版本或语言质量边界无效")
    fingerprints = ["source_sha256", "binary_sha256"] + (["cases_sha256", "protocol_sha256", "evidence_sha256"] if real else [])
    if any(not isinstance(report.get(field), str) or not SHA.fullmatch(report[field]) for field in fingerprints):
        raise ValueError("实验指纹缺失或格式无效")
    if real and any(report.get(field) is not None for field in ("top1", "top5", "mrr", "frequency_quality", "language_model_quality")):
        raise ValueError("无gold实验不得计算语言质量")
    expected = set(CONFIGURATIONS) if real else {f"{method}-{ranker}" for method in ("reference", "exact", "beam1", "beam64")
                                               for ranker in ("structural", "weighted", "bigram", "context")}
    runs = report.get("runs", {})
    if set(runs) != expected:
        raise ValueError("固定配置丢失或多出配置")
    indexed, baseline = {}, None
    metadata = ("id", "family", "input", "kind") if real else ("id", "family", "split", "kind")
    for label, run in runs.items():
        if real:
            method, width, edges, expansions, resident = CONFIGURATIONS[label]
            wanted = {"method": method, "beam_width": width, "max_edges": edges, "max_expansions": expansions,
                      "max_resident_paths": resident, "max_input_bytes": 512, "max_letters": 256,
                      "output_k": 5, "ranker": "Structural"}
            if run.get("configuration") != wanted:
                raise ValueError("固定资源配置不符")
        if (run.get("iterations_per_case") != (30 if real else 100)
                or run.get("warmup_per_case") != (5 if real else 10)):
            raise ValueError("预登记测量次数不符")
        index = index_cases(run["cases"])
        identity = {key: {field: value.get(field) for field in metadata} for key, value in index.items()}
        if baseline is None:
            baseline = identity
        elif baseline != identity:
            raise ValueError("配置之间的案例或元数据不符")
        for row in index.values():
            validate_samples(row, run["iterations_per_case"], real)
        indexed[label] = index
    return indexed


def audit_evidence(path: Path, report: dict) -> dict:
    runtime, evidence, counts = validate_evidence(path)
    if digest(path) != report["evidence_sha256"] or evidence != report["evidence"] or counts != report["lexicon_counts"]:
        raise ValueError("实验和证据描述不匹配")
    files = {"runtime": evidence["runtime"], "build_report": evidence["build_report"]}
    for name in ("records", "notice"):
        if name in evidence:
            files[name] = evidence[name]
    for source in evidence["sources"]:
        files[f"{source['id']}:license"] = source["license"]
        files[f"{source['id']}:audit"] = source["audit"]
    verified = {}
    for name, item in files.items():
        checked = checked_file(item, path.parent, name)
        verified[name] = {"path": str(checked), "sha256": digest(checked), "bytes": checked.stat().st_size}
    return {"evidence_path": str(path.resolve()), "evidence_sha256": digest(path), "verified_files": verified,
            "runtime_sha256": digest(runtime), "counts": counts,
            "meaning": "核对锁定证据一致性；不增加来源权利批准，不升级为正式词库。"}


def length_group(value: str) -> str:
    letters = sum(char.isascii() and char.isalpha() or char in "üÜ" for char in value)
    return next(label for end, label in [(0, "0"), (3, "1-3"), (6, "4-6"), (12, "7-12"), (24, "13-24"), (10**9, "25+")]
                if letters <= end)


def outcome(row: dict) -> str:
    if row["status"] != "ok":
        return "rejected"
    if row["statistics"]["budget"] is not None:
        return "budget"
    if row["statistics"]["beam_pruned"]:
        return "beam_pruned"
    return "complete_nonempty" if row.get("candidates") else "complete_empty"


def group_summary(rows: list[dict]) -> dict:
    stats = [row.get("statistics", {}) for row in rows]
    return {"cases": len(rows), "rejected": sum(row["status"] == "rejected" for row in rows),
            "empty": sum(not row.get("candidates") for row in rows),
            "nonempty": sum(bool(row.get("candidates")) for row in rows),
            "complete": sum(complete(row) for row in rows),
            "budget_reasons": dict(Counter(str(value["budget"]) for value in stats if value.get("budget") is not None)),
            "beam_pruned_cases": sum(value.get("beam_pruned", 0) > 0 for value in stats),
            "beam_pruned_total": sum(value.get("beam_pruned", 0) for value in stats),
            "work": {field: {"sum": sum(value.get(field, 0) for value in stats),
                             "max": max((value.get(field, 0) for value in stats), default=None)}
                     for field in ("graph_edges", "expansions", "resident_high_water")},
            "pooled_samples": quantiles([value for row in rows for value in row["samples_ns"]]),
            "max_case_quantiles": {field: max((row[field] for row in rows), default=None)
                                   for field in ("p50_ns", "p95_ns", "p99_ns", "max_ns")}}


def groups(run: dict, inputs: dict[str, str]) -> dict:
    result = {}
    for category, key in [("kind", lambda row: row["kind"]),
                          ("letter_length", lambda row: length_group(inputs[row["id"]])),
                          ("outcome", outcome)]:
        buckets = defaultdict(list)
        for row in run["cases"]:
            buckets[key(row)].append(row)
        result[category] = {label: group_summary(rows) for label, rows in sorted(buckets.items())}
    return {"all": group_summary(run["cases"]), **result,
            "process_memory": {"peak_bytes": run["peak_process_memory_bytes"], "method": run["memory_method"],
                               "per_case_available": False, "scope": "独立配置进程，含词库/索引/宿主/候选/样本数组"},
            "cold_load_ns": run["load_ns"], "lexicon_entries": run["lexicon_entries"], "trie_nodes": run["trie_nodes"]}


def analyze_report(report: dict, real: bool, inputs: dict[str, str]) -> dict:
    indexed = validate_report(report, real)
    if set(inputs) != set(next(iter(indexed.values()))):
        raise ValueError("输入登记与测量案例不符")
    per_case, failures = [], []
    reference_labels = ["reference"] if real else [f"reference-{ranker}" for ranker in ("structural", "weighted", "bigram", "context")]
    for reference_label in reference_labels:
        suffix = "" if real else reference_label.removeprefix("reference")
        labels = [label for label in indexed if label != reference_label and (real or label.endswith(suffix))]
        for key, reference in indexed[reference_label].items():
            methods = {}
            for label in labels:
                comparison = compare_rows(reference, indexed[label][key], real)
                methods[label] = comparison
                if label.startswith(("exact", "position")) and comparison["full_equivalence"] is False:
                    failures.append(f"{reference_label}/{label}/{key}")
            per_case.append({"id": key, "input": inputs[key], "kind": reference["kind"], "reference": reference_label,
                             "methods": methods})
    secondary = []
    if real:
        for key, reference in indexed["reference"].items():
            if not complete(reference):
                comparison = compare_rows(indexed["exact"][key], indexed["position"][key], True)
                secondary.append({"id": key, "oracle": "same_lineage_exact_not_independent_reference", **comparison})
                if comparison["full_equivalence"] is False:
                    failures.append(f"same-lineage-exact/position/{key}")
    return {"case_count_per_configuration": len(next(iter(indexed.values()))), "failures": failures,
            "per_case": per_case, "secondary_exact_position": secondary,
            "resources": {label: groups(run, inputs) for label, run in report["runs"].items()}}


def paired_row(old: dict, new: dict) -> dict:
    def view(row):
        return {"status": row["status"], "complete": complete(row), "p95_ns": row["p95_ns"],
                "p99_ns": row["p99_ns"], "statistics": row.get("statistics"),
                "candidates": len(row.get("candidates", []))}
    valid = complete(old) and complete(new)
    return {"before": view(old), "after": view(new), "both_complete": valid,
            "p95_ratio_if_both_complete": new["p95_ns"] / old["p95_ns"] if valid and old["p95_ns"] else None,
            "same_graph_edges": old.get("statistics", {}).get("graph_edges") == new.get("statistics", {}).get("graph_edges")}


def history_analysis(history: dict, current: dict) -> dict:
    for field in ("cases_sha256", "protocol_sha256"):
        if history.get(field) != current.get(field):
            raise ValueError("历史探针/协议指纹与当前不符")
    if history["evidence"]["runtime"]["sha256"] != current["evidence"]["runtime"]["sha256"]:
        raise ValueError("历史与当前数据指纹不符")
    old = index_cases(history["runs"]["position"]["cases"])
    position = index_cases(current["runs"]["position"]["cases"])
    exact = index_cases(current["runs"]["exact"]["cases"])
    if set(old) != set(position) or any(any(old[key].get(field) != position[key].get(field) for field in ("input", "family", "kind")) for key in old):
        raise ValueError("历史与当前案例不符")
    fixed = [key for key, row in old.items() if row["p95_ns"] > 5_000_000]
    if len(fixed) != 13:
        raise ValueError("历史固定超限队列不是13例")
    selected = set(fixed) | {key for key, row in position.items() if row["kind"] in ("short", "unicode", "boundary", "ambiguity")}
    rows = [{"id": key, "input": position[key]["input"], "kind": position[key]["kind"],
             "in_fixed_historical_13": key in fixed,
             "historical_position_to_current_position": paired_row(old[key], position[key]),
             "current_exact_to_current_position": paired_row(exact[key], position[key])}
            for key in old if key in selected]
    return {"fixed_historical_p95_over_5ms": fixed, "current_p95_over_5ms": [key for key, row in position.items() if row["p95_ns"] > 5_000_000],
            "cases": rows, "causal_limit": "耗时包含规范化、建图、搜索，无阶段计时；工作量变化不是耗时占比或显著性证明。"}


def run() -> None:
    parser = argparse.ArgumentParser()
    for name in ("mechanical", "unihan", "wiktionary", "unihan-evidence", "wiktionary-evidence"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--history", type=Path, default=ROOT / "docs/reports/cl008-real/unihan-after.json")
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if args.output_dir.exists():
        parser.error("输出目录已存在，禁止覆盖历史证据")
    paths = {name: getattr(args, name) for name in ("mechanical", "unihan", "wiktionary", "history")}
    reports = {name: json.loads(path.read_text(encoding="utf-8")) for name, path in paths.items()}
    if len({reports[name]["source_sha256"] for name in ("mechanical", "unihan", "wiktionary")}) != 1:
        raise ValueError("本轮源码指纹不一致")
    if reports["unihan"]["binary_sha256"] != reports["wiktionary"]["binary_sha256"]:
        raise ValueError("两份真实资源宿主二进制不一致")
    mechanical_cases = read_table(DATA / "cases.tsv", "# cloudleaf-search-cases-v1")
    if reports["mechanical"].get("data_sha256") != {path.name: digest(path) for path in sorted(DATA.glob("*.tsv"))}:
        raise ValueError("机械数据指纹不符")
    resource_cases = validate_cases()
    analysis = {}
    audits = {}
    for name in ("unihan", "wiktionary"):
        if reports[name]["cases_sha256"] != digest(CASES) or reports[name]["protocol_sha256"] != digest(PROTOCOL):
            raise ValueError("真实探针或协议指纹不符")
        actual = [{field: row[field] for field in ("id", "family", "input", "kind")}
                  for row in reports[name]["runs"]["reference"]["cases"]]
        if actual != resource_cases:
            raise ValueError("真实探针顺序或内容不符")
        audits[name] = audit_evidence(getattr(args, name + "_evidence"), reports[name])
        analysis[name] = analyze_report(reports[name], True, {row["id"]: row["input"] for row in resource_cases})
    analysis["mechanical"] = analyze_report(reports["mechanical"], False, {row["id"]: row["input"] for row in mechanical_cases})
    historical = history_analysis(reports["history"], reports["unihan"])
    failures = [f"{name}/{failure}" for name, item in analysis.items() for failure in item["failures"]]
    result = {"schema_version": 1, "experiment": "cloudleaf-search-round3-analysis-v1", "language_accuracy_valid": False,
              "top1": None, "top5": None, "mrr": None, "analyzed_at_utc": datetime.now(timezone.utc).isoformat(),
              "analyzer_sha256": digest(Path(__file__)), "design_sha256": digest(ROOT / "docs/cl008-round3-analysis-design.md"),
              "input_reports": {name: {"path": str(path.resolve()), "sha256": digest(path)} for name, path in paths.items()},
              "source_sha256": reports["unihan"]["source_sha256"], "evidence_audits": audits,
              "analysis": analysis, "historical_diagnostic_cohort": historical, "correctness_failures": failures,
              "limitations": ["没有合法独立gold、正式train/dev/封存heldout，不计算语言准确率。",
                              "两份数据分别为字符资源和有来源顺序偏差的研究摘录，不是正式多字产品词库。",
                              "被剪枝/预算/拒绝的输出仍在分母，完整等价结论为null；相同不完整输出不构成证明。",
                              "进程峰值不能转换成每例内存，驻留路径计数不是字节数；背景负载未隔离。",
                              "历史机械实验没有原始样本，不补造；本轮宿主补齐有序samples_ns。"]}
    args.output_dir.mkdir(parents=True)
    (args.output_dir / "analysis.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output_dir / "analysis.json"), "correctness_failures": failures,
                      "fixed_historical_cases": len(historical["fixed_historical_p95_over_5ms"])}, ensure_ascii=False))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    run()

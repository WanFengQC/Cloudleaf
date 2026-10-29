"""评估固定原型排序，严格保留失败分母、隔离历史回归并记录实际产物。"""

from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DATASET = ROOT / "tests/evaluation/algorithm-v1.tsv"
ALGORITHMS = ("production-rules-v1", "frequency-only-ablation-v1")
FIELDS = ["id", "split", "category", "group", "input", "context", "gold", "correction", "note"]


def historical_inputs(path: Path) -> set[str]:
    """历史事件输入和非空预编辑均被排除，不能换ID伪装盲测。"""
    lines = path.read_text(encoding="utf-8").splitlines()
    excluded: set[str] = set()
    for row in csv.DictReader(lines[1:], delimiter="\t"):
        excluded.add(row["events"].lower())
        if row["preedit"] != "-":
            excluded.add(row["preedit"].lower())
        for token in row["events"].split():
            if token.startswith("text:"):
                excluded.add(token[5:].lower())
    return excluded


def validate_rows(rows: list[dict], excluded: set[str]) -> None:
    if not rows:
        raise ValueError("评估集不能为空")
    ids, examples, groups = set(), set(), {}
    for row in rows:
        if set(row) != set(FIELDS) or any(value is None or not value for value in row.values()):
            raise ValueError("数据必须包含完整的九个非空字段")
        if any(any(ord(char) < 32 or ord(char) == 127 for char in value) for value in row.values()):
            raise ValueError("字段不能包含控制字符")
        if row["split"] not in {"dev", "heldout"} or row["correction"] not in {"clean", "required"}:
            raise ValueError("未知split或correction")
        if row["id"] in ids:
            raise ValueError("ID重复")
        ids.add(row["id"])
        gold = row["gold"].split("|")
        if "" in gold or "-" in gold or len(gold) != len(set(gold)):
            raise ValueError("gold必须为不重复的非空可接受文本")
        key = (row["input"].lower(), row["context"], tuple(sorted(gold)))
        if key in examples:
            raise ValueError("输入、上下文和gold重复")
        examples.add(key)
        if row["input"].lower() in excluded:
            raise ValueError("评估输入与历史事件回归重复")
        previous = groups.setdefault(row["group"], row["split"])
        if previous != row["split"]:
            raise ValueError("同一group不能跨split")


def read_dataset(path: Path, regression: Path) -> list[dict]:
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    if not lines or lines[0] != "# cloudleaf-algorithm-evaluation-v1":
        raise ValueError("数据集版本错误")
    if len(lines) < 2 or lines[1].split("\t") != FIELDS:
        raise ValueError("数据集表头错误")
    rows = list(csv.DictReader(lines[1:], delimiter="\t"))
    validate_rows(rows, historical_inputs(regression))
    return rows


def percentile(values: list[int], percent: int) -> int | None:
    if not values:
        return None
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * percent / 100) - 1)]


def aggregate(rows: list[dict]) -> dict:
    """分母包括所有未命中/未实现/OOV，不把无纠错预测误记为precision=0。"""
    count = len(rows)
    required = sum(row["correction"] == "required" for row in rows)
    attempts = sum(row["correction_attempt"] for row in rows)
    correction_correct = sum(row["correction"] == "required" and row["correction_attempt"] and row["correction_correct"] for row in rows)
    times = [value for row in rows for value in row["durations_ns"]]
    return {
        "count": count,
        "top1_count": sum(row["gold_rank"] == 1 for row in rows),
        "top5_count": sum(1 <= row["gold_rank"] <= 5 for row in rows),
        "top1": sum(row["gold_rank"] == 1 for row in rows) / count if count else None,
        "top5": sum(1 <= row["gold_rank"] <= 5 for row in rows) / count if count else None,
        "mrr": sum(1 / row["gold_rank"] if row["gold_rank"] else 0 for row in rows) / count if count else None,
        "candidate_availability": sum(bool(row["candidates"]) for row in rows) / count if count else None,
        "gold_coverage": sum(row["gold_rank"] > 0 for row in rows) / count if count else None,
        "errors": sum(row["error"] != "-" for row in rows),
        "correction": {"capability": False, "required": required, "attempts": attempts,
                       "correct": correction_correct,
                       "precision": correction_correct / attempts if attempts else None,
                       "recall": correction_correct / required if required else None},
        "latency": {"samples": len(times), "p50_ns": percentile(times, 50),
                    "p95_ns": percentile(times, 95), "p99_ns": percentile(times, 99),
                    "max_ns": max(times) if times else None},
    }


def read_results(path: Path, dataset: list[dict]) -> list[dict]:
    source = {row["id"]: row for row in dataset}
    results, seen = [], set()
    with path.open(encoding="utf-8", newline="") as stream:
        for raw in csv.DictReader(stream, delimiter="\t"):
            key = (raw["id"], raw["algorithm"])
            if raw["id"] not in source or raw["algorithm"] not in ALGORITHMS or key in seen:
                raise ValueError("评估产物包含未知或重复样例/算法")
            seen.add(key)
            row = dict(source[raw["id"]])
            row.update({"algorithm": raw["algorithm"], "candidates": raw["candidates"].split("|") if raw["candidates"] else [],
                        "gold_rank": int(raw["gold_rank"]), "lexicon_known": raw["lexicon_known"] == "true",
                        "correction_attempt": raw["correction_attempt"] == "true",
                        "correction_correct": raw["correction_correct"] == "true", "error": raw["error"],
                        "durations_ns": [int(value) for value in raw["durations_ns"].split(",")]})
            if len(row["durations_ns"]) != 100 or int(raw["samples"]) != 100 or any(value < 0 for value in row["durations_ns"]):
                raise ValueError("计时样本数量或取值错误")
            for name, percent in [("p50_ns", 50), ("p95_ns", 95), ("p99_ns", 99)]:
                if int(raw[name]) != percentile(row["durations_ns"], percent):
                    raise ValueError("逐样例分位数不一致")
            if int(raw["max_ns"]) != max(row["durations_ns"]):
                raise ValueError("逐样例最大值不一致")
            expected_rank = next((index + 1 for index, text in enumerate(row["candidates"]) if text in row["gold"].split("|")), 0)
            if row["gold_rank"] != expected_rank or len(row["candidates"]) != len(set(row["candidates"])):
                raise ValueError("候选排名或去重错误")
            if row["correction_attempt"] or row["correction_correct"]:
                raise ValueError("本轮算法不能声称具有纠错预测能力")
            results.append(row)
    if len(seen) != len(dataset) * len(ALGORITHMS):
        raise ValueError("评估产物缺失样例或算法")
    return results


def source_sha256() -> str:
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml"]
    for folder, pattern in [("crates", "*.rs"), ("adapters", "*.rs"), ("crates", "Cargo.toml"), ("apps", "*.rs"),
                            ("apps", "Cargo.toml"), ("scripts", "*.py"), ("tests", "*.py"), ("tests", "*.tsv"), ("data", "*.tsv")]:
        paths.extend((ROOT / folder).rglob(pattern))
    digest = hashlib.sha256()
    for path in sorted(set(paths), key=lambda item: item.relative_to(ROOT).as_posix()):
        digest.update(path.relative_to(ROOT).as_posix().encode("utf-8") + b"\0" + path.read_bytes() + b"\0")
    return digest.hexdigest()


def build_binary() -> Path:
    build = subprocess.run(["cargo", "build", "--release", "--locked", "--offline", "-p", "cloudleaf-cli",
                            "--bin", "cloudleaf-evaluate", "--message-format=json"], cwd=ROOT,
                           capture_output=True, text=True, encoding="utf-8", check=False)
    if build.returncode:
        raise RuntimeError(f"评估器构建失败（exit {build.returncode}）：\n{build.stderr}\n{build.stdout}")
    binary = None
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if message.get("reason") == "compiler-artifact" and message.get("target", {}).get("name") == "cloudleaf-evaluate" and message.get("executable"):
            binary = Path(message["executable"])
    if binary is None or not binary.is_file():
        raise ValueError("Cargo没有返回实际评估器产物")
    return binary


def write_reports(output: Path, dataset: list[dict], rows: list[dict], provenance: dict) -> dict:
    report = {"schema_version": 1, "dataset_version": "algorithm-v1", "provenance": provenance,
              "dataset": {"count": len(dataset), "splits": {split: sum(row["split"] == split for row in dataset) for split in ("dev", "heldout")}, "train_count": 0, "dev_tuned": False},
              "limitations": ["手工诊断seed不是成熟语言准确率评估", "固定64条原型词典且人工词频", "context被两个算法忽略", "无纠错、模糊音、简拼、双拼、整句生成或学习", "本轮只比较移除Exact优先的排序消融", "heldout首轮查看后不能继续当盲测调参"], "algorithms": {}}
    for algorithm in ALGORITHMS:
        selected = [row for row in rows if row["algorithm"] == algorithm]
        item = {"all": aggregate(selected), "lexicon_known": aggregate([row for row in selected if row["lexicon_known"]]), "by_split": {}, "by_category": {}, "by_group": {}}
        for field in ("split", "category", "group"):
            item[f"by_{field}"] = {value: aggregate([row for row in selected if row[field] == value]) for value in sorted({row[field] for row in selected})}
        report["algorithms"][algorithm] = item
    by_id = {(row["id"], row["algorithm"]): row for row in rows}
    differences = []
    for example in dataset:
        base, ablation = [by_id[example["id"], algorithm] for algorithm in ALGORITHMS]
        # 未命中的名次取候选上限+1，正数表示消融的gold名次更好。
        differences.append({"id": example["id"], "split": example["split"], "category": example["category"],
                            "input": example["input"], "context": example["context"],
                            "gold": example["gold"], "production_rank": base["gold_rank"], "ablation_rank": ablation["gold_rank"],
                            "rank_gain_ablation": (base["gold_rank"] or 51) - (ablation["gold_rank"] or 51),
                            "candidate_order_changed": base["candidates"] != ablation["candidates"],
                            "added_by_ablation": [text for text in ablation["candidates"] if text not in base["candidates"]],
                            "removed_by_ablation": [text for text in base["candidates"] if text not in ablation["candidates"]],
                            "production_candidates": base["candidates"], "ablation_candidates": ablation["candidates"]})
    # 末列保留明确比较状态；空候选列仍为空，不能通过删列消除合法TSV的末尾制表符。
    for comparison in differences:
        gain = comparison["rank_gain_ablation"]
        comparison["gold_rank_comparison"] = (
            "ablation_improved" if gain > 0 else "ablation_regressed" if gain < 0 else "same_gold_rank"
        )
    report["case_comparison"] = differences
    (output / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    with (output / "case-comparison.tsv").open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(differences[0]), delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows({key: "|".join(value) if isinstance(value, list) else value for key, value in row.items()} for row in differences)
    markdown = ["# algorithm-v1 排序消融实测", "", "本报告来自实际Rust核心；只评价手工诊断seed，不是正式产品算法成熟度结论。两个算法都忽略context且没有纠错预测。", "", f"数据：{len(dataset)}条；dev与heldout组隔离，未使用dev调参，train为空。", "", "| 算法 | 分组 | 样例 | Top-1 | Top-5 | MRR | 非空候选率 | gold覆盖 | P95 ms |", "|---|---|---:|---:|---:|---:|---:|---:|---:|"]
    for algorithm in ALGORITHMS:
        item = report["algorithms"][algorithm]
        for group, metrics in [("all", item["all"]), ("lexicon_known", item["lexicon_known"])] + list(item["by_split"].items()) + list(item["by_category"].items()):
            ratio = lambda value: "-" if value is None else f"{value:.4f}"
            latency = metrics["latency"]["p95_ns"]
            markdown.append(f"| {algorithm} | {group} | {metrics['count']} | {ratio(metrics['top1'])} | {ratio(metrics['top5'])} | {ratio(metrics['mrr'])} | {ratio(metrics['candidate_availability'])} | {ratio(metrics['gold_coverage'])} | {latency / 1e6 if latency is not None else '-'} |")
    markdown += ["", "全部失败、OOV、未支持策略、超长与无候选样例均保留在分母。lexicon_known仅表示gold存在于固定词典，不代表当前输入可检索到它。纠错零预测precision=null；required样例recall=0。JSON包含按group及split/category的完整指标，raw-results.tsv保留100次计时样本，case-comparison.tsv逐项保留候选与排名变化。", "", "| 样例 | split | gold | 现有规则rank | 频率消融rank | 消融rank改善 | 候选顺序改变 |", "|---|---|---|---:|---:|---:|---|"]
    for item in differences:
        markdown.append(f"| {item['id']} | {item['split']} | {item['gold'].replace('|', ' / ')} | {item['production_rank']} | {item['ablation_rank']} | {item['rank_gain_ablation']} | {item['candidate_order_changed']} |")
    markdown += ["", "排名0表示未命中，改善值计算时按51处理；它不是平均质量收益的替代指标。不同方案需在更大独立词库/语料、长句和真实平台上对照，不能只凭seed或这一次消融决定最终模型。", "", "## 产物来源", "", "```json", json.dumps(provenance, ensure_ascii=False, indent=2), "```", ""]
    (output / "report.md").write_text("\n".join(markdown), encoding="utf-8", newline="\n")
    return report


def run() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, default=ROOT / "docs/reports/algorithm-v1")
    args = parser.parse_args()
    dataset = read_dataset(DATASET, ROOT / "tests/fixtures/input-v1.tsv")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    before = source_sha256()
    binary = build_binary()
    raw = args.output_dir / "raw-results.tsv"
    subprocess.run([str(binary), "--dataset", str(DATASET), "--output", str(raw)], cwd=ROOT, check=True)
    if source_sha256() != before:
        raise ValueError("构建或评估期间源码变化，拒绝生成来源错配报告")
    rows = read_results(raw, dataset)
    provenance = {"measured_at_utc": datetime.now(timezone.utc).isoformat(), "source_sha256": before,
                  "source_fingerprint_schema": "cloudleaf-source-v3",
                  "dataset_sha256": hashlib.sha256(DATASET.read_bytes()).hexdigest(),
                  "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                  "dictionary_sha256": hashlib.sha256((ROOT / "data/dictionaries/prototype.tsv").read_bytes()).hexdigest(),
                  "rustc": subprocess.check_output(["rustc", "--version"], encoding="utf-8").strip(),
                  "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, encoding="utf-8").strip(),
                  "git_tracked_dirty": bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"], cwd=ROOT, encoding="utf-8").strip()),
                  "environment": {"os": platform.platform(), "cpu": platform.processor(), "machine": platform.machine()},
                  "profile": "release", "warmup_per_case": 20, "measurements_per_case": 100, "candidate_limit": 50,
                  "context_used": False, "correction_capability": False}
    report = write_reports(args.output_dir, dataset, rows, provenance)
    print(json.dumps({algorithm: report["algorithms"][algorithm]["by_split"] for algorithm in ALGORITHMS}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    run()

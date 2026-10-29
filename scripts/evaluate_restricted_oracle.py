"""在冻结真实子词库上独立枚举全部路径；结构参照不是语言gold。"""
from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time

from analyze_search_round3 import audit_evidence, compare_rows, complete, digest, quantiles
from benchmark import environment, source_sha256
from evaluate_real_lexicon import HEADER, build_host, validate_evidence
from evaluate_search import measure

ROOT = Path(__file__).resolve().parents[1]
PROTOCOL = ROOT / "docs/cl008-round4-evaluation-gates.md"
READINGS = ("a", "an", "na", "nan", "xi", "xian", "shi", "zhong", "guo", "bei", "jing", "xue", "lv")
INPUTS = ("", "ana", "anana", "xian", "xi'an", "shishi", "zhongguo", "beijing", "xuexi", "lü", "zhongg", "zhong'", "zzzz", "ni,hao")
CONFIGURATIONS = {"reference": ("reference", 64, 200000, 50000), "exact": ("exact", 64, 200000, 50000),
                  "position": ("position", 64, 200000, 50000), "beam1": ("beam", 1, 200000, 50000),
                  "beam8": ("beam", 8, 200000, 50000), "beam64": ("beam", 64, 200000, 50000),
                  "exact-expansions1": ("exact", 64, 1, 50000), "position-expansions1": ("position", 64, 1, 50000),
                  "exact-resident1": ("exact", 64, 200000, 1), "position-resident1": ("position", 64, 200000, 1)}


def load_rows(path: Path) -> list[dict]:
    with path.open(encoding="utf-8", newline="") as stream:
        if stream.readline() != "# cloudleaf-lexicon-v2\n":
            raise ValueError("只接受已核验LF v2词条")
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != HEADER:
            raise ValueError("词条表头无效")
        return list(reader)


def select_rows(rows: list[dict]) -> tuple[list[dict], dict]:
    selected = []
    counts = {}
    for reading in READINGS:
        matching = sorted((row for row in rows if row["pinyin"] == reading), key=lambda row: row["id"])
        chosen = matching[:2]
        counts[reading] = {"available": len(matching), "selected_ids": [row["id"] for row in chosen]}
        selected.extend(chosen)
    if not selected or any(row["frequency_kind"] != "unknown" or row["frequency"] for row in selected):
        raise ValueError("本轮独立Structural参考要求真实unknown频率，不推断词频")
    return sorted(selected, key=lambda row: row["id"]), counts


def normalize(raw: str) -> tuple[str, set[int], bool]:
    if len(raw.encode("utf-8")) > 512:
        raise ValueError("InvalidInput")
    letters, separators, waiting = "", set(), False
    for char in raw:
        if char == "'":
            if not letters or waiting:
                raise ValueError("InvalidInput")
            separators.add(len(letters))
            waiting = True
        elif "a" <= char <= "z" or "A" <= char <= "Z" or char in "üÜ":
            letters += "v" if char in "üÜ" else char.lower()
            waiting = False
        else:
            raise ValueError("InvalidInput")
        if len(letters) > 256:
            raise ValueError("InvalidInput")
    return letters, separators, waiting


def exhaustive(rows: list[dict], raw: str, expansion_limit: int = 200000, edge_limit: int = 16384) -> dict:
    # 不使用Rust图或候选。朴素逐词字符串匹配与全部路径枚举独立实现语义。
    try:
        letters, separators, waiting = normalize(raw)
    except ValueError:
        return {"status": "rejected", "error_kind": "InvalidInput"}
    outgoing = [[] for _ in range(len(letters) + 1)]
    edge_count, budget = 0, None
    for start in range(len(letters)) if not waiting else ():
        for entry in rows:
            tokens = entry["pinyin"].split(" ")
            reading = "".join(tokens)
            boundaries, size = set(), 0
            for token in tokens[:-1]:
                size += len(token)
                boundaries.add(size)
            remaining = letters[start:]
            options = []
            if remaining.startswith(reading):
                options.append((start + len(reading), False))
            if (len(reading) > len(remaining) > max(boundaries, default=0)
                    and reading.startswith(remaining)):
                options.append((len(letters), True))
            for end, prefix in options:
                if any(start < mark < end and mark - start not in boundaries for mark in separators):
                    continue
                edge_count += 1
                if edge_count > edge_limit:
                    budget = "IndependentGraphEdges"
                    break
                outgoing[start].append((end, entry, prefix))
            if budget:
                break
        if budget:
            break
    paths = []
    pending = [(0, [])] if letters and not waiting and budget is None else []
    expansions, peak = 0, len(pending)
    while pending:
        position, path = pending.pop()
        if position == len(letters):
            paths.append(path)
            continue
        for end, entry, prefix in outgoing[position]:
            if expansions >= expansion_limit:
                budget = "IndependentExpansions"
                pending.clear()
                break
            expansions += 1
            pending.append((end, path + [(entry, prefix)]))
            peak = max(peak, len(pending) + len(paths))
    candidates = []
    for path in paths:
        entries, prefix_flags = zip(*path)
        prefixes = sum(prefix_flags)
        candidates.append({"text": "".join(entry["text"] for entry in entries), "score": -100 * len(path) - 300 * prefixes,
                           "ids": [entry["id"] for entry in entries],
                           "records": [{field: entry[field] for field in ("id", "source", "source_record_id", "license_id")} for entry in entries],
                           "features": {"words": len(path), "prefixes": prefixes, "frequency_units": 0,
                                        "missing_frequencies": len(path), "bigram_units": 0, "context_units": 0}})
    candidates.sort(key=lambda row: (-row["score"], row["ids"]))
    unique, texts = [], set()
    for candidate in candidates:
        if candidate["text"] not in texts:
            unique.append(candidate)
            texts.add(candidate["text"])
    return {"status": "ok", "candidates": unique[:5], "guaranteed": budget is None, "waiting": waiting,
            "fallback_preserved": True, "whole_entry_complete": budget != "IndependentGraphEdges",
            "statistics": {"graph_edges": min(edge_count, edge_limit), "expansions": expansions,
                           "resident_high_water": peak, "beam_pruned": 0, "budget": budget}, "enumerated_complete_paths": len(paths)}


def python_worker(runtime: Path, cases: Path) -> dict:
    rows = load_rows(runtime)
    probes = list(csv.DictReader(cases.read_text(encoding="utf-8").splitlines()[1:], delimiter="\t"))
    output = []
    for probe in probes:
        for _ in range(5):
            exhaustive(rows, probe["input"])
        samples = []
        for _ in range(30):
            started = time.perf_counter_ns()
            exhaustive(rows, probe["input"])
            samples.append(time.perf_counter_ns() - started)
        result = exhaustive(rows, probe["input"])
        output.append({**probe, **result, "samples_ns": samples, **{key: value for key, value in quantiles(samples).items() if key != "samples"}})
    return {"oracle": "independent-python-string-matching-exhaustive", "language_accuracy_valid": False,
            "iterations_per_case": 30, "warmup_per_case": 5, "cases": output}


def run() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--python-worker", nargs=2, type=Path)
    args = parser.parse_args()
    if args.python_worker:
        print(json.dumps(python_worker(*args.python_worker), ensure_ascii=False))
        return
    if not args.evidence or not args.output_dir or args.output_dir.exists():
        parser.error("须提供证据和新的空输出目录，禁止覆盖历史")
    runtime, evidence, counts = validate_evidence(args.evidence)
    evidence_audit = audit_evidence(args.evidence, {"evidence": evidence, "evidence_sha256": digest(args.evidence), "lexicon_counts": counts})
    before = source_sha256()
    selected, selection = select_rows(load_rows(runtime))
    folder = args.output_dir
    folder.mkdir(parents=True)
    subset, cases = folder / "restricted-runtime.tsv", folder / "cases.tsv"
    with subset.open("w", encoding="utf-8", newline="") as stream:
        stream.write("# cloudleaf-lexicon-v2\n")
        writer = csv.DictWriter(stream, fieldnames=HEADER, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(selected)
    with cases.open("w", encoding="utf-8", newline="") as stream:
        stream.write("# cloudleaf-search-resource-cases-v1\nid\tfamily\tinput\tkind\n")
        for index, value in enumerate(INPUTS):
            stream.write(f"subset-{index:02}\trestricted-structure\t{value}\tdictionary-derived-resource-probe\n")
    binary = build_host()
    runs = {}
    for label, (method, width, expansions, resident) in CONFIGURATIONS.items():
        command = [str(binary), str(subset), str(cases), method, str(width), "30", "5", "16384", str(expansions), str(resident), "5"]
        result, peak, memory_method = measure(command)
        runs[label] = {**result, "command": command, "peak_process_memory_bytes": peak, "memory_method": memory_method,
                       "configuration": {"method": method, "beam_width": width, "max_edges": 16384,
                                         "max_expansions": expansions, "max_resident_paths": resident, "ranker": "Structural", "output_k": 5}}
    command = [sys.executable, str(Path(__file__)), "--python-worker", str(subset), str(cases)]
    independent, peak, memory_method = measure(command)
    independent.update(command=command, peak_process_memory_bytes=peak, memory_method=memory_method)
    comparisons, failures = [], []
    for index, oracle in enumerate(independent["cases"]):
        methods = {}
        for label, result in runs.items():
            candidate = result["cases"][index]
            if candidate["id"] != oracle["id"]:
                raise ValueError("Rust探针案例缺失或顺序不符")
            item = compare_rows(oracle, candidate, True)
            methods[label] = item
            if label.startswith(("reference", "exact", "position")) and item["full_equivalence"] is False:
                failures.append(f"{label}:{oracle['id']}")
        comparisons.append({"id": oracle["id"], "input": oracle["input"], "oracle_complete": complete(oracle), "methods": methods})
    audit_evidence(args.evidence, {"evidence": evidence, "evidence_sha256": digest(args.evidence), "lexicon_counts": counts})
    if source_sha256() != before:
        raise RuntimeError("实验期间源码/资源发生变化")
    result = {"schema_version": 1, "experiment": "cloudleaf-independent-restricted-oracle-v1", "language_accuracy_valid": False,
              "top1": None, "top5": None, "mrr": None, "dictionary_derived_probe": True, "product_data": False,
              "measured_at_utc": datetime.now(timezone.utc).isoformat(), "environment": environment(),
              "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, encoding="utf-8").strip(),
              "source_sha256": before, "binary_sha256": digest(binary), "oracle_sha256": digest(Path(__file__)),
              "protocol_sha256": digest(PROTOCOL), "subset_sha256": digest(subset), "cases_sha256": digest(cases),
              "evidence_audit": evidence_audit, "selection": selection, "selected_entries": len(selected),
              "independent_oracle": independent, "runs": runs, "comparisons": comparisons, "failures": failures,
              "limitations": ["按预登记规则缩小的真实子词库，不是完整词库/全图验证或人工语言gold。",
                              "独立参考不使用Rust候选或图；仅验证当前Structural规则，不评估词频、模型或语言质量。",
                              "Python与Rust进程峰值含不同宿主运行时，不据耗时比宣称生产收益；背景负载未控制。"]}
    (folder / "results.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    shutil.copyfile(args.evidence, folder / "source-evidence.json")
    for index, source in enumerate(evidence["sources"]):
        license_path = evidence_audit["verified_files"][f"{source['id']}:license"]["path"]
        shutil.copyfile(license_path, folder / f"LICENSE-{index + 1}.txt")
    print(json.dumps({"output": str(folder), "selected_entries": len(selected), "failures": failures}, ensure_ascii=False))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    run()

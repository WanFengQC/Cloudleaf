"""隔离机械训练快照，运行同图搜索对照及独立进程性能测量。"""
from __future__ import annotations

import argparse
import csv
import ctypes
from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time

from benchmark import environment, source_sha256

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tests/search-mechanics-v1"


def read_table(path: Path, magic: str) -> list[dict[str, str]]:
    text = path.read_text(encoding="utf-8")
    first, separator, body = text.partition("\n")
    if not separator or first != magic or "\r" in text or text.startswith("\ufeff"):
        raise ValueError("机械资源版本或编码无效")
    reader = csv.DictReader(io.StringIO(body), delimiter="\t")
    rows = list(reader)
    if any(None in row or None in row.values() for row in rows):
        raise ValueError("机械资源字段数无效")
    return rows


def validate_isolation(folder: Path = DATA) -> dict:
    train = read_table(folder / "train.tsv", "# cloudleaf-search-training-v1")
    cases = read_table(folder / "cases.tsv", "# cloudleaf-search-cases-v1")
    families = {"train": {row["family"] for row in train}, "dev": set(), "test": set()}
    ids: set[str] = set()
    normalized: dict[str, str] = {}
    for row in cases:
        if row["id"] in ids or row["split"] not in ("dev", "test"):
            raise ValueError("重复ID或非法划分")
        ids.add(row["id"])
        families[row["split"]].add(row["family"])
        key = row["input"].lower().replace("ü", "v")
        if key in normalized and normalized[key] != row["split"]:
            raise ValueError("相同输入跨dev/test泄漏")
        normalized[key] = row["split"]
    if any(families[left] & families[right] for left, right in [("train", "dev"), ("train", "test"), ("dev", "test")]):
        raise ValueError("训练和评估家族交叉")
    return {"families": {key: sorted(value) for key, value in families.items()},
            "case_counts": {split: sum(row["split"] == split for row in cases) for split in ("dev", "test")},
            "training_rows": len(train), "training_sequences": sum(int(row["multiplicity"]) for row in train),
            "language_accuracy_valid": False,
            "old_83_seed_used_for_training_or_tuning": False,
            "mechanical_only": True}


def build_host() -> Path:
    build = subprocess.run(["cargo", "build", "--release", "--locked", "--offline", "-p", "cloudleaf-search",
                            "--example", "search_probe", "--message-format=json"], cwd=ROOT,
                           check=False, capture_output=True, text=True, encoding="utf-8")
    artifacts = []
    for line in build.stdout.splitlines():
        item = json.loads(line)
        if item.get("reason") == "compiler-message":
            print(item["message"].get("rendered", ""), file=sys.stderr, end="")
        if (item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "search_probe"
                and item.get("executable")):
            artifacts.append(Path(item["executable"]))
    if build.returncode or len(artifacts) != 1 or not artifacts[0].is_file():
        raise RuntimeError(f"搜索评估宿主构建失败：{build.stderr}")
    return artifacts[0]


def measure(command: list[str], *, stdout_path: Path | None = None, stderr_path: Path | None = None) -> tuple[dict, int | None, str]:
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        process = subprocess.Popen(command, cwd=ROOT, stdout=output, stderr=errors)
        peak, memory_method = None, "unavailable"
        if os.name == "nt":
            class Counters(ctypes.Structure):
                _fields_ = [("cb", ctypes.c_ulong), ("faults", ctypes.c_ulong)] + [
                    (name, ctypes.c_size_t) for name in ["peak_working", "working", "peak_paged", "paged",
                                                       "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile"]]
            counters = Counters()
            counters.cb = ctypes.sizeof(counters)
            getter = ctypes.windll.psapi.GetProcessMemoryInfo
            getter.argtypes = [ctypes.c_void_p, ctypes.POINTER(Counters), ctypes.c_ulong]
            getter.restype = ctypes.c_int
            while True:
                if getter(int(process._handle), ctypes.byref(counters), counters.cb):
                    peak = max(peak or 0, counters.peak_working)
                    memory_method = "Windows GetProcessMemoryInfo.PeakWorkingSetSize"
                if process.poll() is not None:
                    break
                time.sleep(0.01)
            process.wait()
        else:
            _, status, usage = os.wait4(process.pid, 0)
            process.returncode = os.waitstatus_to_exitcode(status)
            peak = usage.ru_maxrss * (1 if platform.system() == "Darwin" else 1024)
            memory_method = "wait4(probe_pid).ru_maxrss"
        output.seek(0)
        errors.seek(0)
        output_bytes, error_bytes = output.read(), errors.read()
        if stdout_path is not None:
            stdout_path.write_bytes(output_bytes)
        if stderr_path is not None:
            stderr_path.write_bytes(error_bytes)
        if process.returncode:
            raise RuntimeError(error_bytes.decode("utf-8"))
        return json.loads(output_bytes.decode("utf-8")), peak, memory_method


def compare_results(runs: dict) -> dict:
    comparisons = {}
    for ranker in ("structural", "weighted", "bigram", "context"):
        reference = {case["id"]: case for case in runs[f"reference-{ranker}"]["cases"]}
        for method in ("exact", "beam64", "beam1"):
            differences = []
            candidate_cases = runs[f"{method}-{ranker}"]["cases"]
            if len(candidate_cases) != len(reference) or {case["id"] for case in candidate_cases} != set(reference):
                raise ValueError("对照方法丢失案例或出现重复ID")
            for case in candidate_cases:
                baseline = reference[case["id"]]
                fields = ("status", "error_kind", "candidates", "fallback_preserved", "waiting")
                if any(case.get(field) != baseline.get(field) for field in fields):
                    differences.append(case["id"])
            comparisons[f"{method}-{ranker}"] = {"reference_different_cases": differences, "total_cases": len(reference)}
            if method == "exact" and differences:
                raise ValueError(f"精确搜索与参考不一致：{ranker} {differences}")
    return comparisons


def run() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--iterations", type=int, default=100)
    parser.add_argument("--warmup", type=int, default=10)
    parser.add_argument("--output", type=Path, default=ROOT / ".local/cl008-search.json")
    args = parser.parse_args()
    if not 1 <= args.iterations <= 10000 or not 0 <= args.warmup <= 1000:
        parser.error("测量次数超限")
    isolation = validate_isolation()
    before = source_sha256()
    binary = build_host()
    runs = {}
    for label, method, width in [("reference", "reference", 64), ("exact", "exact", 64),
                                  ("beam64", "beam", 64), ("beam1", "beam", 1)]:
        for ranker in ("structural", "weighted", "bigram", "context"):
            command = [str(binary), str(DATA / "lexicon.tsv"), str(DATA / "train.tsv"), str(DATA / "cases.tsv"),
                       method, ranker, str(args.iterations), str(args.warmup), str(width), "5"]
            result, peak, memory_method = measure(command)
            result["peak_process_memory_bytes"] = peak
            result["memory_method"] = memory_method
            runs[f"{label}-{ranker}"] = result
    comparisons = compare_results(runs)
    if source_sha256() != before:
        raise RuntimeError("构建/评估期间源码或资源发生变化")
    result = {"schema_version": 1, "experiment": "cloudleaf-search-mechanics-v1", "language_accuracy_valid": False,
              "measured_at_utc": datetime.now(timezone.utc).isoformat(), "environment": environment(),
              "rustc": subprocess.check_output(["rustc", "--version"], encoding="utf-8").strip(),
              "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, encoding="utf-8").strip(),
              "source_fingerprint_schema": "cloudleaf-source-v2", "source_sha256": before,
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "build_profile": "release",
              "data_sha256": {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(DATA.glob("*.tsv"))},
              "isolation": isolation, "comparisons": comparisons, "runs": runs,
              "limitations": "机械符号、手工特征及合成转移计数；没有正式语言准确率，未控制背景负载；峰值含宿主、模型及样本数组；不是生产词库或TSF延迟。"}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "comparisons": comparisons, "language_accuracy_valid": False}, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    run()

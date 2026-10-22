"""python -m tools.data_pipeline 的显式离线/取得命令。"""

from __future__ import annotations

import argparse
import ctypes
import json
import math
import platform
from pathlib import Path
import statistics
import sys
import time

from .pipeline import activate, build, builder_fingerprint, digest, fetch, read_json, rollback, verify_release, write_json


def peak_memory_bytes() -> int | None:
    if sys.platform == "win32":
        class Counters(ctypes.Structure):
            _fields_ = [("cb", ctypes.c_ulong), ("faults", ctypes.c_ulong)] + [
                (name, ctypes.c_size_t) for name in ("peak", "working", "peak_paged", "paged", "peak_nonpaged",
                                                     "nonpaged", "pagefile", "peak_pagefile")]
        counters = Counters()
        counters.cb = ctypes.sizeof(counters)
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        psapi.GetProcessMemoryInfo.argtypes = [ctypes.c_void_p, ctypes.POINTER(Counters), ctypes.c_ulong]
        if psapi.GetProcessMemoryInfo(kernel.GetCurrentProcess(), ctypes.byref(counters), counters.cb):
            return counters.peak
        return None
    try:
        import resource
        value = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        return value if sys.platform == "darwin" else value * 1024
    except ImportError:
        return None


def benchmark(release: Path, iterations: int) -> dict:
    manifest = verify_release(release)
    start = time.perf_counter_ns()
    index = read_json(release / "index.json")
    load_ns = time.perf_counter_ns() - start
    keys = sorted(index["by_pinyin"])
    samples, result_count = [], 0
    for number in range(iterations):
        key = keys[number % len(keys)]
        start = time.perf_counter_ns()
        values = index["by_pinyin"].get(key, [])
        elapsed = time.perf_counter_ns() - start
        result_count += len(values)
        samples.append(elapsed)
    samples.sort()
    return {"scope": "Python离线JSON索引载入和精确键查询；不是Rust候选、TSF或语言准确率",
            "python": sys.version.split()[0], "platform": platform.platform(), "machine": platform.machine(),
            "processor": platform.processor(), "iterations": iterations, "index_bytes": (release / "index.json").stat().st_size,
            "manifest_sha256": digest((release / "manifest.json").read_bytes()),
            "builder_files_sha256": builder_fingerprint(),
            "release_builder_files_sha256": manifest["builder_files_sha256"],
            "release_builder_matches_current": manifest["builder_files_sha256"] == builder_fingerprint(),
            "load_ms": load_ns / 1e6, "query_ns": {"p50": statistics.median(samples),
            "p95": samples[math.ceil(len(samples) * .95) - 1],
            "p99": samples[math.ceil(len(samples) * .99) - 1], "max": max(samples)},
            "peak_process_bytes": peak_memory_bytes(), "memory_method": "OS进程峰值工作集/RSS（包括前置完整校验）",
            "result_count": result_count, "background_load": "未隔离桌面后台负载"}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    get = commands.add_parser("fetch")
    get.add_argument("--lock", type=Path, required=True)
    get.add_argument("--output", type=Path, required=True)
    create = commands.add_parser("build")
    create.add_argument("--lock", type=Path, required=True)
    create.add_argument("--source", type=Path, required=True)
    create.add_argument("--license", type=Path, required=True)
    create.add_argument("--output", type=Path, required=True)
    check = commands.add_parser("verify")
    check.add_argument("release", type=Path)
    install = commands.add_parser("activate")
    install.add_argument("release", type=Path)
    install.add_argument("--store", type=Path, required=True)
    undo = commands.add_parser("rollback")
    undo.add_argument("--store", type=Path, required=True)
    bench = commands.add_parser("benchmark")
    bench.add_argument("release", type=Path)
    bench.add_argument("--iterations", type=int, default=10000)
    bench.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "fetch":
        print(fetch(args.lock, args.output))
    elif args.command == "build":
        start = time.perf_counter()
        result = build(args.lock, args.source, args.license, args.output)
        print(json.dumps({"entries": result["entry_count"], "rejected": result["rejected_count"],
                          "build_seconds": time.perf_counter() - start,
                          "peak_process_bytes": peak_memory_bytes()}, ensure_ascii=False))
    elif args.command == "verify":
        result = verify_release(args.release)
        print(json.dumps({"verified": True, "entries": result["entry_count"], "source": result["source"]["source_id"]}))
    elif args.command == "activate":
        print(json.dumps(activate(args.release, args.store)))
    elif args.command == "rollback":
        print(json.dumps(rollback(args.store)))
    else:
        if args.iterations < 1:
            raise ValueError("iterations必须为正")
        result = benchmark(args.release, args.iterations)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        write_json(args.output, result)
        print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"数据管线失败：{error}", file=sys.stderr)
        sys.exit(1)

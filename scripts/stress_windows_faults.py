"""显式运行异常占用下的健康会话压力；仅采样自己创建的测试进程，不注册或激活输入法。"""

from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import threading
import time

from benchmark import source_sha256

ROOT = Path(__file__).resolve().parents[1]
TEST = ("native::session::render_tests::"
        "round5_abrupt_owner_workers_reclaim_slots_before_healthy_fifo_session")


class ThreadEntry(ctypes.Structure):
    _fields_ = [("size", wintypes.DWORD), ("usage", wintypes.DWORD),
                ("thread_id", wintypes.DWORD), ("process_id", wintypes.DWORD),
                ("base", wintypes.LONG), ("delta", wintypes.LONG), ("flags", wintypes.DWORD)]


class ProcessMemory(ctypes.Structure):
    _fields_ = [("size", wintypes.DWORD), ("faults", wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in ["peak_working", "working", "peak_paged", "paged",
                                             "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile", "private"]]


def collector():
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessMemory), wintypes.DWORD]
    psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
    kernel.GetProcessHandleCount.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    kernel.GetProcessHandleCount.restype = wintypes.BOOL
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.Thread32First.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    kernel.Thread32First.restype = wintypes.BOOL
    kernel.Thread32Next.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    kernel.Thread32Next.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL

    def memory(process):
        counters = ProcessMemory()
        counters.size = ctypes.sizeof(counters)
        if not psapi.GetProcessMemoryInfo(int(process._handle), ctypes.byref(counters), counters.size):
            raise ctypes.WinError(ctypes.get_last_error())
        return {"working_set_bytes": counters.working, "os_peak_working_set_bytes": counters.peak_working,
                "private_commit_bytes": counters.private}

    def sample(process):
        handles = wintypes.DWORD()
        if not kernel.GetProcessHandleCount(int(process._handle), ctypes.byref(handles)):
            raise ctypes.WinError(ctypes.get_last_error())
        snapshot = kernel.CreateToolhelp32Snapshot(4, 0)
        if snapshot == ctypes.c_void_p(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            entry = ThreadEntry()
            entry.size = ctypes.sizeof(entry)
            okay = kernel.Thread32First(snapshot, ctypes.byref(entry))
            count = 0
            while okay:
                count += entry.process_id == process.pid
                okay = kernel.Thread32Next(snapshot, ctypes.byref(entry))
            if ctypes.get_last_error() != 18:
                raise ctypes.WinError(ctypes.get_last_error())
            return {"handles": handles.value, "threads": count, **memory(process)}
        finally:
            kernel.CloseHandle(snapshot)

    return sample, memory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--seconds", type=int, choices=range(180, 601), default=180)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("此显式工程诊断只支持Windows")
    binary = args.test_binary.resolve(strict=True)
    out = args.output_dir.resolve()
    out.mkdir(parents=True, exist_ok=False)
    command = [str(binary), TEST, "--exact", "--nocapture", "--test-threads=1"]
    env = os.environ.copy()
    env["CLOUDLEAF_RECOVERY_REVIEW_CHILD"] = TEST
    env["CLOUDLEAF_REVIEW_PINNED_SECONDS"] = str(args.seconds)
    sample, memory = collector()
    started = time.perf_counter()
    samples, events = [], []
    errors = []
    with (out / "stdout.txt").open("w", encoding="utf-8", newline="") as log:
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True, encoding="utf-8")

        def read_output():
            for line in process.stdout:
                log.write(line)
                log.flush()
                offset = line.find('{"experiment":')
                if offset >= 0:
                    try:
                        event = json.loads(line[offset:])
                        events.append(event)
                        print(json.dumps(event, ensure_ascii=False), flush=True)
                    except json.JSONDecodeError:
                        pass

        reader = threading.Thread(target=read_output)
        reader.start()
        timed_out = False
        while process.poll() is None:
            elapsed = time.perf_counter() - started
            if elapsed > args.seconds + 60:
                # 只终止本脚本拥有的故障测试子进程，保留失败日志和已取得样本。
                timed_out = True
                process.kill()
                break
            try:
                samples.append({"elapsed_seconds": elapsed, **sample(process)})
            except OSError as error:
                errors.append({"elapsed_seconds": elapsed, "error": str(error)})
            time.sleep(0.2)
        code = process.wait()
        try:
            final_memory = memory(process)
        except OSError as error:
            final_memory = None
            errors.append({"stage": "retained handle after child exit", "error": str(error)})
        reader.join(timeout=10)
        if reader.is_alive():
            raise RuntimeError("自有测试进程输出读取未结束")
    final = next((event for event in reversed(events)
                  if event["experiment"] == "r5-pinned-availability"), None)
    report = {
        "schema_version": 1,
        "implementation_commit": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_sha256": source_sha256(),
        "source_fingerprint_schema": "cloudleaf-source-v3",
        "binary": str(binary), "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "command": command,
        "explicit_environment": {name: env[name] for name in
                                 ["CLOUDLEAF_RECOVERY_REVIEW_CHILD", "CLOUDLEAF_REVIEW_PINNED_SECONDS"]},
        "os": platform.platform(), "pid": process.pid, "exit_code": code,
        "timeout": timed_out, "requested_seconds": args.seconds,
        "wall_seconds": time.perf_counter() - started,
        "measurement": "Own child: GetProcessHandleCount + Toolhelp thread snapshot; 200ms samples",
        "memory_method": "GetProcessMemoryInfo.PROCESS_MEMORY_COUNTERS_EX: sampled WorkingSetSize/PrivateUsage; OS-recorded PeakWorkingSetSize, retained child handle after exit",
        "final_memory": final_memory,
        "samples": samples, "sampling_errors": errors, "events": events,
        "first": samples[0] if samples else None, "last": samples[-1] if samples else None,
        "sampled_peak": {name: max((row[name] for row in samples), default=None)
                         for name in ["handles", "threads", "working_set_bytes", "private_commit_bytes"]},
        "mechanism_passed": bool(code == 0 and not timed_out and final and
                                 final["pinned_budget_unchanged"] and
                                 final["steady_elapsed_ns"] >= args.seconds * 1_000_000_000),
        "system_registration_executed": False, "real_application_acceptance": False,
        "long_term_production_reliability_accepted": False,
    }
    (out / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
                                      encoding="utf-8", newline="\n")
    print(json.dumps({name: report[name] for name in ["exit_code", "wall_seconds", "first", "last",
                                                      "sampled_peak", "mechanism_passed"]}), flush=True)
    return 0 if report["mechanism_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

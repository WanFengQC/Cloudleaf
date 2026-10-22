"""运行 release 基准，记录环境、源码校验及独立进程峰值内存。"""

from __future__ import annotations

import argparse
import ctypes
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def source_sha256() -> str:
    # 源码尚未提交时也有可重复追溯的标识；排除 target/.git 与生成的报告。
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml"]
    for folder, pattern in [("crates", "*.rs"), ("adapters", "*.rs"), ("crates", "Cargo.toml"),
                            ("apps", "*.rs"), ("apps", "Cargo.toml"), ("tests", "*.py"), ("tests", "*.tsv"),
                            ("data", "*.tsv"), ("scripts", "*.py")]:
        paths.extend((ROOT / folder).rglob(pattern))
    digest = hashlib.sha256()
    for path in sorted(set(paths), key=lambda item: item.relative_to(ROOT).as_posix()):
        digest.update(path.relative_to(ROOT).as_posix().encode("utf-8") + b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def environment() -> dict:
    info = {"os": platform.platform(), "machine": platform.machine(),
            "cpu": platform.processor(), "logical_cpus": os.cpu_count()}
    if os.name == "nt":
        import winreg
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE,
                           r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") as key:
            info["cpu"] = winreg.QueryValueEx(key, "ProcessorNameString")[0].strip()

        class MemoryStatus(ctypes.Structure):
            _fields_ = [("length", ctypes.c_ulong), ("load", ctypes.c_ulong)] + [
                (name, ctypes.c_ulonglong) for name in
                ["total_phys", "avail_phys", "total_page", "avail_page", "total_virtual", "avail_virtual", "extended"]]

        memory = MemoryStatus()
        memory.length = ctypes.sizeof(memory)
        if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(memory)):
            info["physical_memory_bytes"] = memory.total_phys
    elif hasattr(os, "sysconf"):
        info["physical_memory_bytes"] = os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")
    return info


def measure(binary: Path, iterations: int) -> tuple[dict, int | None, str]:
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        return measure_process(binary, iterations, output, errors)


def measure_process(binary: Path, iterations: int, output, errors) -> tuple[dict, int | None, str]:
    process = subprocess.Popen([str(binary), "--benchmark", str(iterations)],
                               cwd=ROOT, stdout=output, stderr=errors)
    peak_bytes = None
    method = "unavailable"
    if os.name == "nt":
        class ProcessMemoryCounters(ctypes.Structure):
            _fields_ = [("cb", ctypes.c_ulong), ("faults", ctypes.c_ulong)] + [
                (name, ctypes.c_size_t) for name in
                ["peak_working", "working", "peak_paged", "paged", "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile"]]

        counters = ProcessMemoryCounters()
        counters.cb = ctypes.sizeof(counters)
        get_memory = ctypes.windll.psapi.GetProcessMemoryInfo
        get_memory.argtypes = [ctypes.c_void_p, ctypes.POINTER(ProcessMemoryCounters), ctypes.c_ulong]
        get_memory.restype = ctypes.c_int
        # Windows 保留的进程句柄允许在退出前后读取操作系统记录的峰值，而非采样最大 RSS。
        while True:
            if get_memory(int(process._handle), ctypes.byref(counters), counters.cb):
                peak_bytes = max(peak_bytes or 0, counters.peak_working)
                method = "Windows GetProcessMemoryInfo.PeakWorkingSetSize"
            if process.poll() is not None:
                break
            time.sleep(0.01)
        process.wait()
    else:
        # 直接读取这一进程的资源，不能混入先前 cargo/rustc 子进程的内存峰值。
        _, status, usage = os.wait4(process.pid, 0)
        process.returncode = os.waitstatus_to_exitcode(status)
        value = usage.ru_maxrss
        peak_bytes = value if platform.system() == "Darwin" else value * 1024
        method = "wait4(benchmark_pid).ru_maxrss"
    output.seek(0)
    errors.seek(0)
    stdout = output.read().decode("utf-8")
    stderr = errors.read().decode("utf-8")
    if process.returncode:
        raise RuntimeError(f"基准失败：{stderr}")
    return json.loads(stdout), peak_bytes, method


def build_host() -> Path:
    # 以 Cargo 实际构建产物为准，兼容 CARGO_TARGET_DIR 与显式目标平台配置。
    build = subprocess.run(
        ["cargo", "build", "--release", "--locked", "--offline", "-p", "cloudleaf-cli",
         "--bin", "cloudleaf-cli", "--message-format=json"],
        cwd=ROOT, check=False, capture_output=True, text=True, encoding="utf-8",
    )
    if build.returncode:
        for line in build.stdout.splitlines():
            message = json.loads(line)
            if message.get("reason") == "compiler-message":
                print(message["message"].get("rendered", ""), file=sys.stderr, end="")
        print(build.stderr, file=sys.stderr, end="")
        raise subprocess.CalledProcessError(build.returncode, build.args)
    paths = []
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if (message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "cloudleaf-cli"
                and "bin" in message.get("target", {}).get("kind", [])
                and message.get("executable")):
            paths.append(Path(message["executable"]))
    if len(paths) != 1 or not paths[0].is_file():
        raise RuntimeError("Cargo 未返回唯一且存在的 cloudleaf-cli 产物")
    return paths[0]


def run() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--iterations", type=int, default=1000)
    parser.add_argument("--output", type=Path, default=ROOT / ".local/m2-performance.json")
    args = parser.parse_args()
    if not 1 <= args.iterations <= 10000:
        parser.error("iterations 必须在 1..10000")
    source_hash = source_sha256()
    binary = build_host()
    result, peak, method = measure(binary, args.iterations)
    if source_sha256() != source_hash:
        raise RuntimeError("构建或测量期间源码发生变化，请重新运行基准")
    result["environment"] = environment()
    result["rustc"] = subprocess.check_output(["rustc", "--version"], encoding="utf-8").strip()
    result["git_head"] = subprocess.run(["git", "rev-parse", "--verify", "HEAD"], cwd=ROOT,
                                         capture_output=True, text=True, encoding="utf-8").stdout.strip() or None
    result["source_sha256"] = source_hash
    result["source_fingerprint_schema"] = "cloudleaf-source-v3"
    result["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
    result["measured_at_utc"] = datetime.now(timezone.utc).isoformat()
    result["build_profile"] = "release"
    result["peak_process_memory_bytes"] = peak
    result["memory_method"] = method
    result["limitations"] = "64 条手工原型词典；空闲/当前背景负载未控制；进程内存包含运行时和样本数组；不含 TSF/UI；不是生产词库成绩。"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    run()

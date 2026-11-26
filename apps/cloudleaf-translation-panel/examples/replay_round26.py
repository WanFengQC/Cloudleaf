"""通过独立面板设置和控制器回放固定 Marian 输出。明细只写入 .local。"""

import argparse
import ctypes
from ctypes import wintypes
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/performance"))
import round25_translation_replay as baseline


def is_gone(pid):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.WaitForSingleObject.argtypes = (wintypes.HANDLE, wintypes.DWORD)
    kernel.WaitForSingleObject.restype = wintypes.DWORD
    kernel.CloseHandle.argtypes = (wintypes.HANDLE,)
    kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.OpenProcess(0x00100000, False, pid)
    if not handle:
        return ctypes.get_last_error() == 87
    try:
        return kernel.WaitForSingleObject(handle, 0) == 0
    finally:
        kernel.CloseHandle(handle)


def replay(probe, worker, output):
    catalog, models = baseline.verify_catalog()
    glossary = baseline.GLOSSARY
    baseline.verify(glossary, baseline.GLOSSARY_SHA)
    previous = ROOT / ".local/round25-translation-final"
    index = baseline.read(previous / "run-index.json")
    source_files, expected, cases = [], {}, []
    for number, take in zip(baseline.CASE_SHA, index["runs"], strict=True):
        fixture = baseline.fixture(number)
        stdout = previous / f"r{number}-stdout.txt"
        baseline.verify(stdout, take["stdout_sha256"])
        prior, errors = baseline.obs(stdout)
        assert not errors
        source_files.extend((fixture, stdout))
        for mode, case, direction, _, source, _ in baseline.observed_rows(fixture).values():
            if mode != "real":
                continue
            key = f"r{number}/{case}"
            cases.append({
                "id": key, "source": source,
                "direction": {"zh-en": "chinese_english", "en-zh": "english_chinese"}[direction],
            })
            expected[key] = prior[case]

    assert len(cases) == 63
    cases.sort(key=lambda case: case["direction"])
    output.mkdir(parents=True)
    staged_probe = output / "worker_replay.exe"
    staged_worker = output / "cloudleaf-translation-worker.exe"
    shutil.copyfile(probe, staged_probe)
    shutil.copyfile(worker, staged_worker)
    case_file = output / "cases.json"
    baseline.write(case_file, cases)
    log = baseline.record(
        output, "panel", [staged_probe, catalog, glossary, case_file, output / "settings"],
        [*source_files, glossary, catalog, case_file, staged_probe, staged_worker, Path(__file__)],
    )
    import json
    rows = [
        json.loads(line)
        for line in (output / "panel-stdout.txt").read_text(encoding="utf-8").splitlines()
    ]
    results = {row["id"]: row for row in rows if row["event"] == "result"}
    comparisons = {}
    for key, prior in expected.items():
        result = results.get(key)
        comparisons[key] = {
            "raw_match": bool(result and result["raw"] == prior["raw"]),
            "findings_match": bool(result and result["findings"] == prior["findings"]),
            "review_match": bool(result and result["review"] == prior["review"]),
            "model_match": bool(result and result["model"] == prior["metrics"]["model"]),
        }
    events = [row["event"] for row in rows if row["event"] != "result"]
    stopped = rows[-1]
    pids = {row["pid"] for row in results.values()}
    pids.update(row["pid"] for row in rows if row["event"] == "corrupt" and row["pid"])
    exited = {str(pid): is_gone(pid) for pid in pids}
    data = {
        "command": log["command"], "returncode": log["exit_code"],
        "stdout_sha256": log["stdout_sha256"], "stderr_sha256": log["stderr_sha256"],
        "probe_sha256": baseline.digest(staged_probe), "worker_sha256": baseline.digest(staged_worker),
        "catalog_sha256": baseline.CATALOG_SHA, "models": models, "cases": comparisons,
        "events": events, "process_exit": exited,
        "warm_reuse_count": sum(row["load_ns"] == 0 for row in results.values()),
    }
    baseline.write(output / "comparison.json", data)
    assert set(results) == set(expected)
    assert all(all(row.values()) for row in comparisons.values())
    assert events == ["idle", "cancelled", "missing", "corrupt", "stopped"]
    assert stopped["pid"] is None
    assert all(exited.values())
    print(f"{len(results)}/63 identical outputs and checks; worker exits verified")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("probe", "worker", "output"):
        parser.add_argument(arg, type=lambda path: Path(path).resolve())
    args = parser.parse_args()
    if not args.output.is_relative_to(ROOT / ".local"):
        parser.error("原始回放仅写入仓库 .local")
    replay(args.probe, args.worker, args.output)

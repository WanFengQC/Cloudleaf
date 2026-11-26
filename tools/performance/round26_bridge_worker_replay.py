"""用固定本地 Marian 资源重放无窗口 Bridge，保存完整 JSONL 和故障证据。"""

import argparse
import ctypes
from ctypes import wintypes
from hashlib import file_digest
import json
import os
from pathlib import Path
from queue import Queue
import shutil
import subprocess
from threading import Thread
import time


def sha256(path):
    with path.open("rb") as stream:
        return file_digest(stream, "sha256").hexdigest()


class Bridge:
    def __init__(self, executable, catalog, catalog_sha, lexicon, output):
        self.output = output
        self.command = [str(executable), str(lexicon), sha256(lexicon), str(catalog), catalog_sha,
                        "--show-text"]
        (output / "command.json").write_text(json.dumps(self.command) + "\n", encoding="utf-8")
        self.stderr = (output / "stderr.log").open("wb")
        self.process = subprocess.Popen(self.command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=self.stderr, creationflags=subprocess.CREATE_NO_WINDOW)
        self.events = Queue()
        self.reader = Thread(target=self.collect, daemon=True)
        self.reader.start()

    def collect(self):
        with (self.output / "stdout.jsonl").open("wb") as log:
            for frame in self.process.stdout:
                log.write(frame)
                log.flush()
                self.events.put(frame)
        self.events.put(None)

    def send(self, op, **values):
        line = json.dumps({"op": op, **values}, ensure_ascii=False).encode("utf-8") + b"\n"
        with (self.output / "stdin.jsonl").open("ab") as log:
            log.write(line)
        self.process.stdin.write(line)
        self.process.stdin.flush()
        frame = self.events.get(timeout=135)
        assert frame, (op, self.process.poll(), self.output)
        record = json.loads(frame)
        assert record["op"] == op, record
        return record

    def until(self, test):
        deadline = time.monotonic() + 30.0
        while time.monotonic() < deadline:
            record = self.send("poll")
            if test(record):
                return record
            if record["state"] == "Failed":
                break
            time.sleep(0.01)
        raise AssertionError(("poll condition not met", record))

    def close(self):
        try:
            self.process.stdin.close()
            code = self.process.wait(timeout=12)
        except subprocess.TimeoutExpired:
            self.process.kill()
            code = self.process.wait(timeout=5)
        self.reader.join(timeout=2)
        self.stderr.close()
        (self.output / "exit-code.txt").write_text(str(code) + "\n", encoding="utf-8")
        return code


class VerifiedWorker:
    def __init__(self, pid, executable):
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        signatures = {
            "OpenProcess": ([wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
            "QueryFullProcessImageNameW": ([wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR,
                                            ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL),
            "TerminateProcess": ([wintypes.HANDLE, wintypes.UINT], wintypes.BOOL),
            "WaitForSingleObject": ([wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            "CloseHandle": ([wintypes.HANDLE], wintypes.BOOL),
        }
        for name, (args, result) in signatures.items():
            function = getattr(kernel, name)
            function.argtypes, function.restype = args, result
        self.kernel = kernel
        terminate, query_image, synchronize = 0x0001, 0x1000, 0x100000
        self.handle = kernel.OpenProcess(terminate | query_image | synchronize, False, pid)
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            path = ctypes.create_unicode_buffer(32768)
            size = wintypes.DWORD(len(path))
            if not kernel.QueryFullProcessImageNameW(self.handle, 0, path, ctypes.byref(size)):
                raise ctypes.WinError(ctypes.get_last_error())
            assert os.path.samefile(path.value, executable), (path.value, executable)
        except BaseException:
            self.close()
            raise

    def terminate(self):
        if not self.kernel.TerminateProcess(self.handle, 26):
            raise ctypes.WinError(ctypes.get_last_error())
        self.wait()

    def wait(self):
        assert self.kernel.WaitForSingleObject(self.handle, 5000) == 0

    def close(self):
        self.kernel.CloseHandle(self.handle)


def output_dir(root, name):
    out = root / name
    out.mkdir()
    return out


def expect(record, state, failure=None):
    assert record["state"] == state and record["translation_failure"] == failure, record
    return record


def script(args, out):
    expected = args.catalog_sha
    assert sha256(args.catalog) == expected
    worker = args.bridge.with_name("cloudleaf-translation-worker.exe")
    host = Bridge(args.bridge, args.catalog, expected, args.lexicon, output_dir(out, "normal"))
    first_child = None
    recovered_child = None
    try:
        expect(host.send("activate"), "Idle")
        typed = expect(host.send("type", text="nihao"), "Idle")
        assert typed["candidate_count"] > 0 and typed["worker"]["pid"] is None, typed
        host.send("candidate", index=0)
        zh_cold = expect(host.send("wait"), "Ready")
        assert zh_cold["original"] == "你好" and zh_cold["translation"], zh_cold
        assert zh_cold["worker"]["metrics"]["load_ns"] > 0 and zh_cold["integrity"] is not None
        assert zh_cold["worker"]["matches_result"], zh_cold
        assert zh_cold["worker"]["model_identity"].startswith("opus-mt-zh-en"), zh_cold
        first_child = VerifiedWorker(zh_cold["worker"]["pid"], worker)
        confirmed = expect(host.send("confirm"), "CommitPending")
        refused = expect(host.send("not-applied"), "RetryReady")
        assert refused["document"] == "" and refused["commit"] == confirmed["commit"]
        retry = expect(host.send("retry"), "CommitPending")
        assert retry["commit"] != confirmed["commit"]
        committed = expect(host.send("applied"), "Completed")
        assert committed["document"] == zh_cold["translation"] and not committed["learning_selection"]

        host.send("type", text="nihao")
        host.send("candidate", index=0)
        zh_warm = expect(host.send("wait"), "Ready")
        assert zh_warm["translation"] == zh_cold["translation"]
        assert zh_warm["worker"]["pid"] == zh_cold["worker"]["pid"]
        assert zh_warm["worker"]["metrics"]["load_ns"] == 0
        assert zh_warm["worker"]["request"]["generation"] == zh_cold["worker"]["request"]["generation"]
        assert zh_warm["worker"]["request"]["request_id"] > zh_cold["worker"]["request"]["request_id"]
        expect(host.send("cancel"), "Cancelled")
        assert host.send("confirm")["error"] == "StaleConfirmation"
        host.send("escape")

        host.send("english", text="Hello!")
        host.send("request-english")
        en_cold = expect(host.send("wait"), "Ready")
        assert en_cold["original"] == "Hello!" and en_cold["translation"], en_cold
        assert en_cold["worker"]["model_identity"].startswith("opus-mt-en-zh"), en_cold
        assert en_cold["worker"]["metrics"]["load_ns"] > 0
        assert en_cold["worker"]["pid"] == zh_cold["worker"]["pid"]

        host.send("english", text="Please do not change those two orders before next Friday.")
        host.send("request-english")
        host.until(lambda row: row["worker"] and row["worker"]["stage"] == "Translating")
        cancelled = expect(host.send("cancel"), "Cancelled")
        assert cancelled["original"].startswith("Please") and cancelled["translation"] is None
        host.until(lambda row: row["worker"] and row["worker"]["stage"] in ("Ready", "Failed"))
        expect(host.send("poll"), "Cancelled")
        host.send("request-english")
        en_retried = expect(host.send("wait"), "Ready")
        assert en_retried["translation"]
        stale = expect(host.send("english", text="Good afternoon!"), "Stale")
        assert stale["translation"] is None and host.send("confirm")["error"] == "StaleConfirmation"

        host.send("request-english")
        host.until(lambda row: row["worker"] and row["worker"]["stage"] == "Translating")
        first_child.terminate()
        killed = host.send("wait")
        assert killed["state"] == "Failed" and killed["translation"] is None, killed
        assert killed["original"] == "Good afternoon!" and killed["worker"]["pid"] is None, killed
        assert killed["translation_failure"] in {
            "OfflineWorker(BrokenPipe)", "OfflineWorker(WorkerExited)"
        }, killed
        assert host.send("poll")["worker"]["request"] == killed["worker"]["request"]
        assert host.send("confirm")["error"] == "StaleConfirmation"

        host.send("request-english")
        recovered = expect(host.send("wait"), "Ready")
        assert recovered["worker"]["request"]["generation"] > killed["worker"]["request"]["generation"]
        assert recovered["worker"]["pid"] != zh_cold["worker"]["pid"]
        recovered_child = VerifiedWorker(recovered["worker"]["pid"], worker)
        expect(host.send("confirm"), "CommitPending")
        unknown = expect(host.send("unknown"), "CommitUnknown")
        blocked = expect(host.send("retry"), "CommitUnknown")
        assert unknown["document"] == committed["document"] and blocked["error"] == "OutcomeUnknown"
        reset = host.send("reset-unknown", session=2)
        assert reset["error"] is None and reset["document"] == committed["document"]
    finally:
        code = host.close()
        if first_child:
            try:
                first_child.wait()
            finally:
                first_child.close()
        if recovered_child:
            try:
                recovered_child.wait()
            finally:
                recovered_child.close()
        assert code == 0, code

    isolated = output_dir(out, "no-worker")
    shutil.copyfile(args.bridge, isolated / args.bridge.name)
    damaged = output_dir(out, "damaged-catalog")
    shutil.copyfile(args.catalog, damaged / args.catalog.name)
    for path in args.catalog.parent.iterdir():
        if path.is_dir():
            dest = damaged / path.name
            dest.mkdir()
            for resource in path.iterdir():
                if resource.suffix == ".safetensors":
                    (dest / resource.name).write_bytes(b"broken Round 26 test asset")
                elif resource.is_file():
                    shutil.copyfile(resource, dest / resource.name)
    cases = [
        ("missing", args.bridge, out / "never-present.json", expected, {"ModelNotFound"}),
        ("hash-mismatch", args.bridge, args.catalog, "0" * 64, {"OfflineCatalogMismatch"}),
        ("bad-weights", args.bridge, damaged / args.catalog.name, expected,
         {"OfflineWorker(ModelInvalid)", "ModelLoadFailed"}),
        ("missing-worker", isolated / args.bridge.name, args.catalog, expected,
         {"OfflineWorker(InvalidWorkerPath)"}),
    ]
    failures = {}
    for name, executable, catalog, catalog_sha, errors in cases:
        host = Bridge(executable, catalog, catalog_sha, args.lexicon, output_dir(out, name))
        try:
            host.send("activate")
            host.send("english", text="Hello!")
            host.send("request-english")
            observed = host.send("wait")
            assert observed["state"] == "Failed" and observed["translation_failure"] in errors, observed
            assert observed["original"] == "Hello!" and observed["translation"] is None
            host.send("chinese")
            typed = host.send("type", text="nihao")
            assert typed["candidate_count"] > 0 and typed["error"] is None, typed
            failures[name] = observed
        finally:
            code = host.close()
            assert code == 0, code
    events = {"zh_cold": zh_cold, "zh_warm": zh_warm, "en_cold": en_cold, "cancelled": cancelled,
              "en_retried": en_retried, "stale": stale, "killed": killed, "recovered": recovered,
              "unknown": unknown, "failures": failures}
    identity = {str(path): sha256(path) for path in
                (args.bridge, worker, args.catalog, args.lexicon, Path(__file__))}
    return {"identity": identity, "events": events}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("bridge", "catalog", "lexicon", "out"):
        parser.add_argument(f"--{flag}", type=Path, required=True)
    parser.add_argument("--catalog-sha", required=True)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("this test validates a Windows child process handle")
    for flag in ("bridge", "catalog", "lexicon", "out"):
        setattr(args, flag, getattr(args, flag).resolve())
    args.out.mkdir(parents=True)
    try:
        report = script(args, args.out)
    except Exception:
        import traceback
        (args.out / "failure.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
    (args.out / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
                                        encoding="utf-8")


if __name__ == "__main__":
    main()

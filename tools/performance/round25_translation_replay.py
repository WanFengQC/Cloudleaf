"""用固定 Marian 重放 Round 23–25。原始源文本及译文仅保存在本地。"""

import argparse
import hashlib
import json
import math
import os
import platform
import re
import subprocess
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / ".local"
OLD = LOCAL / "round24-translation/paired"
FIXED = LOCAL / "target-round24-translation/release"
FIXTURES = ROOT / "tests/fixtures"
CASE_SHA = {
    23: "ed3ed5da44807d58d16c99acede4230e84f5323b34c4d0ea0f636baa09500c26",
    24: "198f4ba4d420be82dfde3ff32f34549b42e8c6d274c7098eb9f56882ab472776",
    25: "8cca3878bb76d88f92d1c7ac7c6100cd639689f188a7515a0161b698a5e5d195",
}
WORKER_SHA = "6c52a4842612b77f53a9efa9e0504b55134c508fdf8a8e7eee041fec92d8cefc"
IPC_SHA = "d8d1f8818b1dae54263279a12ec0f9e3fa5444b3291a816cff79c78c0d62cbba"
OLD_PROBE_SHA = "4181381163cc427ff694954cf8536093a8a4876ce282591399a4391afb91b219"
GLOSSARY_SHA = "d78faa1594a104e7ccf05274cab92f592d7345f9e70cc27af25c83f22be96381"
CATALOG_SHA = "f88a5f57513a0aaae3bbd659950adf042c8c8dde07736084cb951f61f22c7f0f"
WEIGHT_SHA = {
    "zh-en": "a7c75c919608ea1af3bb6d4aca052af43f35e6967a4f133a865bfe2bde13f21e",
    "en-zh": "e5f4423b70cb0f5a40d38d1a2733a77cec2aa23cc0bae647985fe17f8f5fe616",
}
HISTORICAL_SHA = {
    23: "e6317dd5ae2e86b129891c85db32bbe0e9b42c55784c2972953d3a236b6ab26d",
    24: "a091709021867bcf29ff7e46ec98da057c8a6e4259eba8ab38296a0b911f3930",
}
PROBE = ROOT / "tools/performance/round23_translation_probe.rs"
GLOSSARY = FIXTURES / "round23-translation/glossary.tsv"
WORKER = OLD / "cloudleaf-translation-worker.exe"
OLD_PROBE = OLD / "round24-translation-probe.exe"
IPC = FIXED / "libcloudleaf_translation_ipc.rlib"
ESCAPE = re.compile(r"\\([\\nrt])")


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def verify(path, expected):
    value = digest(path)
    if value != expected:
        raise ValueError(f"{Path(path).name} SHA-256 不匹配")
    return value


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write(path, value):
    with Path(path).open("w", encoding="utf-8", newline="\n") as out:
        out.write(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def head():
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip()


def record(out, label, command, inputs=(), environment=None):
    command = list(map(str, command))
    path = out / f"{label}.json"
    if path.exists():
        raise FileExistsError(path)
    data = {
        "command": command,
        "head": head(),
        "environment": platform.platform(),
        "start_utc": datetime.now(UTC).isoformat(),
        "inputs_sha256": {str(path): digest(path) for path in inputs},
    }
    with (out / f"{label}-stdout.txt").open("xb") as stdout, (out / f"{label}-stderr.txt").open("xb") as stderr:
        try:
            result = subprocess.run(command, cwd=ROOT, check=False, env=environment, stdout=stdout, stderr=stderr,
                                    timeout=250, creationflags=subprocess.CREATE_NO_WINDOW)
            data["exit_code"] = result.returncode
        except subprocess.TimeoutExpired:
            data["exit_code"] = 124
    data["end_utc"] = datetime.now(UTC).isoformat()
    data["stdout_sha256"] = digest(out / f"{label}-stdout.txt")
    data["stderr_sha256"] = digest(out / f"{label}-stderr.txt")
    write(path, data)
    print(label, data["exit_code"], flush=True)
    if data["exit_code"]:
        raise SystemExit(data["exit_code"])
    return data


def verify_catalog():
    source = read(LOCAL / "round23-translation-verified/run.json")
    path = Path(source["arguments"][2])
    verify(path, CATALOG_SHA)
    results = {}
    for model in read(path)["models"]:
        manifest = path.parent / model["manifest"]
        verify(manifest, model["sha256"])
        data = read(manifest)
        assert data["license_status"] == "research_only" and data["architecture"] == "marian"
        artifacts = {}
        for file in data["files"]:
            artifact = manifest.parent / file["path"]
            assert artifact.stat().st_size == file["bytes"]
            artifacts[file["role"]] = verify(artifact, file["sha256"])
        assert artifacts["weights"] == WEIGHT_SHA[data["direction"]]
        results[data["direction"]] = {
            "revision": data["revision"], "manifest_sha256": model["sha256"], "files": artifacts,
        }
    assert set(results) == set(WEIGHT_SHA)
    return path, results


def fixture(round_no):
    path = FIXTURES / f"round{round_no}-translation/cases.tsv"
    verify(path, CASE_SHA[round_no])
    return path


def obs(path):
    observations, metrics, models, failures = {}, {}, {}, {}
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        fields = line.split("\t")
        if fields[0] == "OBSERVATION":
            _, mode, key, topic, warn, findings, escaped = fields
            assert key not in observations
            output = ESCAPE.sub(lambda m: {"n": "\n", "r": "\r", "t": "\t", "\\": "\\"}[m[1]], escaped)
            observations[key] = {"mode": mode, "topic": topic, "review": warn == "true",
                                 "findings": findings.split(",") if findings else [], "raw": output}
        elif fields[0] == "MODEL":
            _, key, model, catalog, infer, load, threads = fields
            assert key not in models
            models[key] = {
                "model": model, "catalog_sha256": catalog, "inference_ns": int(infer),
                "load_ns": int(load), "threads": int(threads),
            }
        elif fields[0] == "METRICS":
            _, key, steps, truncated, verified, enc, dec, queue, provider = fields
            assert key not in metrics
            metrics[key] = {"steps": int(steps), "truncated": truncated == "true",
                            "verification_ns": int(verified), "encoder_ns": int(enc), "decoder_ns": int(dec),
                            "queue_wait_ns": int(queue), "provider": provider}
        elif fields[0] == "FAILED":
            _, key, failure = fields
            failures[key] = failure
        else:
            raise ValueError("探针输出中存在未知记录")
    for key, model in models.items():
        observations[key].update(metrics=metrics[key] | model)
    return observations, failures


def fixed_worker_record():
    return verify(WORKER, WORKER_SHA)


def compile_probe(out):
    verify(IPC, IPC_SHA)
    fixed_worker_record()
    verify(OLD_PROBE, OLD_PROBE_SHA)
    verify(GLOSSARY, GLOSSARY_SHA)
    for number, sha in HISTORICAL_SHA.items():
        verify(OLD / f"r{number}-stdout.txt", sha)
    sources = sorted((ROOT / "crates/cloudleaf-translation-integrity/src").glob("*.rs"))
    pin = (Path.home() / ".rustup/toolchains/1.90.0-x86_64-pc-windows-msvc/bin/rustc.exe")
    cargo = Path.home() / ".cargo/bin/cargo.exe"
    live_dir = out / "target"
    env = dict(os.environ, CARGO_TARGET_DIR=str(live_dir), CARGO_BUILD_JOBS="2")
    source_hashes = [*sources, ROOT / "Cargo.lock", ROOT / "Cargo.toml", PROBE]
    record(out, "build", [cargo, "+1.90.0", "build", "--release", "--locked", "--offline",
                          "-p", "cloudleaf-translation-integrity"], source_hashes, env)
    crate = live_dir / "release/libcloudleaf_translation_integrity.rlib"
    exe = out / "round25-probe.exe"
    record(out, "compile", [
        pin, "--edition", "2024", "-C", "lto=thin", "-C", "opt-level=3", "-C", "debuginfo=1",
        "-L", f"dependency={FIXED / 'deps'}", "-L", f"dependency={live_dir / 'release/deps'}",
        "--extern", f"cloudleaf_translation_integrity={crate}",
        "--extern", f"cloudleaf_translation_ipc={IPC}", PROBE, "-o", exe,
    ], [PROBE, crate, IPC])
    write(out / "freeze.json", {
        "head": head(), "source_sha256": {str(path): digest(path) for path in source_hashes},
        "probe_sha256": digest(exe), "runner_sha256": digest(__file__), "worker_sha256": WORKER_SHA,
        "rounds": {str(round_no): digest(fixture(round_no)) for round_no in CASE_SHA},
        "frozen_utc": datetime.now(UTC).isoformat(),
    })


def observed_rows(path):
    return {
        fields[1]: fields
        for fields in (line.split("\t") for line in path.read_text(encoding="utf-8").splitlines()[1:])
    }


def replays(out):
    frozen = read(out / "freeze.json")
    for path, sha in frozen["source_sha256"].items():
        verify(path, sha)
    verify(__file__, frozen["runner_sha256"])
    verify(GLOSSARY, GLOSSARY_SHA)
    exe = out / "round25-probe.exe"
    verify(exe, frozen["probe_sha256"])
    verify(OLD_PROBE, OLD_PROBE_SHA)
    worker = fixed_worker_record()
    catalog, models = verify_catalog()
    runs = []
    for number in CASE_SHA:
        cases = fixture(number)
        pins = [WORKER, catalog, cases, GLOSSARY]
        run = record(out, f"r{number}", [exe, "--detailed", WORKER, catalog, CATALOG_SHA, cases, GLOSSARY],
                     [exe, *pins])
        runs.append(run)
    cases = fixture(25)
    actual, failed = obs(out / "r25-stdout.txt")
    copied = out / "same-raw-r25.tsv"
    authored = observed_rows(cases)
    simple = all(data["raw"] != "-" and not any(char in data["raw"] for char in "\t\r\n")
                 for data in actual.values() if data["mode"] == "real")
    if simple:
        with copied.open("x", encoding="utf-8", newline="\n") as stream:
            stream.write("mode\tid\tdirection\ttopic\tsource\tintentional_output\n")
            for key, fields in authored.items():
                if key in failed:
                    continue
                if fields[0] == "real":
                    fields = ["control", *fields[1:5], actual[key]["raw"]]
                stream.write("\t".join(fields) + "\n")
    else:
        # 旧探针的控制列接受字面 TSV。含分隔符时直接回放原始请求，再核对译文身份。
        copied = cases
    runs.append(record(out, "legacy-same-raw-r25",
                       [OLD_PROBE, "--detailed", WORKER, catalog, CATALOG_SHA, copied, GLOSSARY],
                       [OLD_PROBE, WORKER, catalog, copied, GLOSSARY]))
    write(out / "run-index.json", {"runs": runs, "worker": worker, "models": models, "freeze": frozen})
    summarize(out)


def counts(pairs):
    old_total = sum(record["old_review"] for record in pairs)
    new_total = sum(record["new_review"] for record in pairs)
    return {"n": len(pairs), "previous_review": old_total, "new_review": new_total}

def timing(rows):
    durations = sorted(row["metrics"]["inference_ns"] for row in rows)
    summary = {f"p{p}_ns": durations[math.ceil(len(rows) * p / 100) - 1] for p in (50, 95, 99, 100)}
    summary["cold_load_ns"] = [row["metrics"]["load_ns"] for row in rows if row["metrics"]["load_ns"]]
    summary["steps"] = sum(row["metrics"]["steps"] for row in rows)
    summary["steps_per_second"] = summary["steps"] * 1e9 / sum(durations)
    summary["truncated"] = sum(row["metrics"]["truncated"] for row in rows)
    return summary


def summarize(out):
    run_index = read(out / "run-index.json")
    history = {}
    all_results = []
    for number in CASE_SHA:
        new, failures = obs(out / f"r{number}-stdout.txt")
        source = OLD / f"r{number}-stdout.txt" if number != 25 else out / "legacy-same-raw-r25-stdout.txt"
        earlier, earlier_failures = obs(source)
        authored = observed_rows(fixture(number))
        assert authored.keys() == (new.keys() | failures.keys())
        assert set(earlier) == set(new), (number, earlier_failures, failures)
        results = []
        for key, current in new.items():
            old = earlier[key]
            assert old["raw"] == current["raw"], key
            result = {
                "case": key, "round": number, "mode": authored[key][0], "direction": authored[key][2],
                "topic": current["topic"], "new_findings": current["findings"], "old_findings": old["findings"],
                "new_review": current["review"], "old_review": old["review"],
                "raw_sha256": hashlib.sha256(current["raw"].encode()).hexdigest(),
                "metrics": current.get("metrics"),
            }
            results.append(result)
        real = [item for item in results if item["mode"] == "real"]
        directions = Counter(item["direction"] for item in real)
        history[number] = {
            "actual": counts(real), "raw_equal": len(real), "directions": directions,
            "timing": {direction: timing([row for row in real if row["direction"] == direction])
                       for direction in directions},
            "controls": counts([item for item in results if item["mode"] == "control"]),
            "failures": failures, "baseline_output_sha256": digest(source),
            "current_output_sha256": digest(out / f"r{number}-stdout.txt"),
        }
        all_results.extend(results)
    suite = [item for item in all_results if item["round"] == 25 and item["mode"] == "control"]
    control = {tag: counts([item for item in suite if item["topic"].startswith(tag + "_")])
               for tag in ("error", "match")}
    write(out / "public-safe.json", {
        "head_at_build": run_index["freeze"]["head"], "head_at_report": head(),
        "probe_sha256": run_index["freeze"]["probe_sha256"], "worker_sha256": WORKER_SHA,
        "catalog_sha256": CATALOG_SHA, "models": run_index["models"], "history": history,
        "prespecified_r25_controls": control, "cases": all_results,
    })
    print(json.dumps({"history": history, "controls": control}), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--summarize", action="store_true")
    args = parser.parse_args()
    out = args.out.resolve()
    if args.prepare:
        out.mkdir(parents=True, exist_ok=False)
        compile_probe(out)
    elif args.summarize:
        summarize(out)
    else:
        replays(out)


if __name__ == "__main__":
    main()

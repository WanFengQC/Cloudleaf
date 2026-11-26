"""在同一搜索预算配对 Round 27 与 Round 28 的输入行为和资源。"""

import argparse
import json
from pathlib import Path
import subprocess
import zipfile

from round27_search_replay import (
    FIXTURES, LOCAL, ROOT, diagnose, get_json, head, identity, metrics, percentile,
    put_json, read_replay, resources, run,
)

BASELINE = "a5675b0e88fb4ea9f0e88061350f4a255beb98c5"
TARGET_LABELS_SHA256 = "13b7202f9b9f617a01f4844c73bac168a63d76fb600197cad8023eec4dfd9865"
TARGET = LOCAL / "round28-search-target"
PROBE_DIR = ROOT / "tools/performance"


def build(out, reuse_baseline=None):
    out.mkdir()
    probe = PROBE_DIR / "round26_input_probe.rs"
    plain_probe = out / "round28_input_plain.rs"
    text = probe.read_text(encoding="utf-8")
    start = text.index("fn allocated(")
    end = text.index("fn identity(")
    text = text[:start] + text[end:]
    text = text.replace("use std::alloc::{GlobalAlloc, Layout, System};\n", "")
    text = text.replace("struct MeasuredAllocator;\n\n", "")
    plain_probe.write_text(text, encoding="utf-8", newline="\n")

    graph_probe = PROBE_DIR / "round27_graph_diagnostics.rs"
    probes = {"input": plain_probe, "alloc": probe, "graph": graph_probe}
    if reuse_baseline is None:
        archive = out / "baseline.zip"
        sources = out / "baseline-source"
        run(out / "archive", [
            "git", "archive", "--format=zip", "-o", archive, BASELINE,
            "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "apps", "crates", "vendor",
            "data/dictionaries/prototype.tsv",
        ], sample=False)
        with zipfile.ZipFile(archive) as zip_source:
            zip_source.extractall(sources)
        comparisons = (("baseline", sources), ("candidate", ROOT))
    else:
        baseline = get_json(reuse_baseline / "baseline/summary.json")
        assert baseline["head"] == BASELINE
        archive = Path(baseline["archive"]["path"])
        assert identity(archive) == baseline["archive"]
        for name, entry in probes.items():
            receipt = baseline["binaries"][name]
            assert identity(entry)["sha256"] == receipt["tool"]["sha256"]
            assert identity(receipt["binary"]["path"]) == receipt["binary"]
        (out / "baseline").mkdir()
        put_json(out / "baseline/summary.json", baseline)
        comparisons = (("candidate", ROOT),)

    for name, source in comparisons:
        folder = out / name
        build_head = BASELINE if name == "baseline" else head()
        source_files = [
            path
            for crate in (
                "cloudleaf-types", "cloudleaf-pinyin", "cloudleaf-input",
                "cloudleaf-search", "cloudleaf-dictionary", "cloudleaf-user", "cloudleaf-candidate",
            )
            for path in (source / "crates" / crate / "src").rglob("*.rs")
        ] + [source / path for path in ("Cargo.toml", "Cargo.lock", "data/dictionaries/prototype.tsv")]
        source_hashes = {
            path.relative_to(source).as_posix(): identity(path)["sha256"] for path in source_files
        }
        compilation = run(folder / "cargo", [
            "cargo", "build", "-p", "cloudleaf-input", "-p", "cloudleaf-search",
            "-p", "cloudleaf-types", "--release", "--locked", "--offline", "-j2",
            "--target-dir", TARGET, "--message-format=json",
        ], cwd=source, sample=False, deadline_seconds=350)
        artifacts = [
            json.loads(line)
            for line in (folder / "cargo/stdout.txt").read_text(encoding="utf-8").splitlines()
        ]
        ring, = {
            file for row in artifacts
            if row["reason"] == "compiler-artifact" and row["target"]["name"] == "ring"
            for file in row["filenames"] if file.endswith(".rlib")
        }
        release = TARGET / "release"
        command = [
            "rustc", "--edition", "2024", "-O", "-C", "lto=thin",
            "-C", "codegen-units=1", "-L", f"dependency={release / 'deps'}",
        ]
        for path in (release / "build").glob("ring-*/out"):
            command += ["-L", f"native={path}"]
        for library in ("cloudleaf_input", "cloudleaf_search", "cloudleaf_types"):
            command += ["--extern", f"{library}={release / f'lib{library}.rlib'}"]
        command += ["--extern", f"ring={ring}"]
        assert source_hashes == {
            path.relative_to(source).as_posix(): identity(path)["sha256"]
            for path in source_files
        }, f"构建中源码变化: {name}"
        binaries = {}
        for kind, entry in probes.items():
            binary = folder / f"round28-{kind}.exe"
            linked = run(folder / f"link-{kind}", [*command, entry, "-o", binary], sample=False)
            binaries[kind] = {"binary": identity(binary), "tool": identity(entry), "execution": linked}
        put_json(folder / "summary.json", {
            "head": build_head,
            "tool": identity(__file__), "archive": identity(archive),
            "source": source_hashes, "build": compilation, "binaries": binaries,
        })


def compare(build_dir, out, repetitions, mode):
    out.mkdir()
    _, runtime, language, args = resources()
    binaries = {
        name: get_json(build_dir / name / "summary.json")["binaries"][mode]["binary"]
        for name in ("baseline", "candidate")
    }
    for binary in binaries.values():
        assert identity(binary["path"]) == binary
    report = {
        "baseline": BASELINE, "head_at_replay": head(), "tool": identity(__file__),
        "runtime": runtime, "language": language, "binaries": binaries,
        "mode": mode, "runs": {},
    }
    for count, fixture in FIXTURES.items():
        for scheme in ("full", "abbreviated"):
            collected = {name: [] for name in binaries}
            observations = []
            for iteration in range(repetitions):
                order = list(binaries) if iteration % 2 == 0 else list(reversed(binaries))
                paired = {}
                for name in order:
                    folder = out / f"{count}-{scheme}-{iteration + 1}-{name}"
                    result = run(folder, [binaries[name]["path"], *args, fixture, scheme, "normal"])
                    rows, acks = read_replay(folder / "stdout.txt")
                    assert len(rows) == count
                    collected[name].extend(rows)
                    observations.append({"label": name, "iteration": iteration + 1, **result})
                    paired[name] = (
                        [row[:5] + row[11:] for row in rows],
                        [[ack[1], ack[3]] for ack in acks],
                    )
                for index, (old, current) in enumerate(
                    zip(paired["baseline"][0], paired["candidate"][0], strict=True), 1
                ):
                    assert old == current, (count, scheme, iteration + 1, index, old, current)
                assert paired["baseline"][1] == paired["candidate"][1], (count, scheme, iteration + 1)
            report["runs"][f"{count}-{scheme}"] = {
                "fixture": identity(fixture), "events_per_version": count * repetitions,
                "projections_identical": True,
                "observations": observations,
                "metrics": {name: metrics(rows) for name, rows in collected.items()},
            }
            put_json(out / "summary.json", report)


def cap_pair(build_dir, out, labels):
    assert identity(labels)["sha256"] == TARGET_LABELS_SHA256, "目标标注与 Round 27 固定原件不一致"
    out.mkdir()
    for name in ("baseline", "candidate"):
        diagnose(build_dir / name, out / name, labels)
    baseline = get_json(out / "baseline/summary.json")
    candidate = get_json(out / "candidate/summary.json")
    records = {}
    for count in FIXTURES:
        name = str(count)
        left = baseline["runs"][name]["truncated"]
        right = candidate["runs"][name]["truncated"]
        assert left == right, (count, "graph diagnostics changed")
        confirmed = [row for row in right if row["target"] != "None"]
        records[name] = {
            "events": count, "edge_cap": len(right), "targets": len(confirmed),
            "targets_fixed_budget_reachable": sum(row["target"] == "Some(true)" for row in confirmed),
            "targets_diagnostic_wider_budget_reachable": sum(
                row["wider_target"] == "Some(true)" for row in confirmed
            ),
            "no_viable_edges": sum(row["viable_edges"] == "0" for row in right),
            "identical_graph_diagnostics": True,
        }
    assert records["171"]["targets"] == 2, "必须有两处已核对目标的位置"
    assert records["171"]["targets_diagnostic_wider_budget_reachable"] == 2
    assert records["243"]["targets"] == 0
    put_json(out / "summary.json", {"tool": identity(__file__), "runs": records})


def profile(build_dir, out):
    out.mkdir()
    _, runtime, language, args = resources()
    report = {"tool": identity(__file__), "runtime": runtime, "language": language, "runs": {}}
    for count, fixture in FIXTURES.items():
        for name in ("baseline", "candidate"):
            folder = out / f"{count}-{name}"
            binary = get_json(build_dir / name / "summary.json")["binaries"]["input"]["binary"]
            assert identity(binary["path"]) == binary
            execution = run(folder, [binary["path"], *args, fixture, "abbreviated", "profile"],
                            deadline_seconds=240)
            rows = (folder / "stdout.txt").read_text(encoding="utf-8").splitlines()
            assert rows[-1] == f"DONE\t{count}"
            phases = {}
            for row in rows:
                if row.startswith("PHASE\t"):
                    _, _, phase, nanoseconds, calls = row.split("\t")
                    phases.setdefault(phase, []).append((int(nanoseconds), int(calls)))
            report["runs"][folder.name] = {
                "fixture": identity(fixture), "binary": binary, "execution": execution,
                "profiled_events": sum(row.startswith("PROFILE\t") for row in rows),
                "phases": {
                    name: {"nanoseconds": percentile([pair[0] for pair in values]),
                           "calls": percentile([pair[1] for pair in values])}
                    for name, values in phases.items()
                },
            }
            put_json(out / "summary.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    builder = subcommands.add_parser("build")
    builder.add_argument("build_dir", type=Path)
    builder.add_argument("--reuse-baseline", type=Path)
    paired = subcommands.add_parser("compare")
    paired.add_argument("build_dir", type=Path)
    paired.add_argument("out", type=Path)
    paired.add_argument("--repetitions", type=int, choices=range(1, 6), default=2)
    paired.add_argument("--mode", choices=("input", "alloc"), default="input")
    caps = subcommands.add_parser("caps")
    caps.add_argument("build_dir", type=Path)
    caps.add_argument("out", type=Path)
    caps.add_argument("labels", type=Path)
    profiler = subcommands.add_parser("profile")
    profiler.add_argument("build_dir", type=Path)
    profiler.add_argument("out", type=Path)
    options = parser.parse_args()
    directories = options.build_dir.resolve()
    if options.command == "build":
        build(directories, options.reuse_baseline)
    elif options.command == "compare":
        compare(directories, options.out.resolve(), options.repetitions, options.mode)
    elif options.command == "caps":
        cap_pair(directories, options.out.resolve(), options.labels.resolve())
    else:
        profile(directories, options.out.resolve())


if __name__ == "__main__":
    main()

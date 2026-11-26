"""固定资源上的输入按键、词条格截断和分配量配对。"""

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / ".local"
sys.path.insert(0, str(LOCAL))
from r21_evidence_common import get_json, head, identity, percentile, put_json, run

BASELINE = "a6e48cbe28d48a010629a65537cfb6743522356a"
TARGET = LOCAL / "round27-search-target"
FIXTURES = {
    171: ROOT / "tests/fixtures/input-round14-events.tsv",
    243: ROOT / "tests/fixtures/round25-worker-input.tsv",
}


def resources():
    receipt = get_json(LOCAL / "round26-delivery-receipt.json")
    assert receipt["head"] == BASELINE
    index = get_json(LOCAL / "round26-input-allocation/summary.json")
    binary = Path(index["binary"]["path"])
    runtime, language = (index[key] for key in ("input", "lexical"))
    for file in (index["binary"], runtime, language):
        assert identity(file["path"])["sha256"] == file["sha256"]
    args = [runtime["path"], runtime["sha256"], language["path"], language["sha256"]]
    return binary, runtime, language, args


def build(out):
    out.mkdir(exist_ok=False)
    the_head = head()
    compilation = run(
        out / "cargo",
        [
            "cargo", "build", "-p", "cloudleaf-input", "-p", "cloudleaf-search",
            "-p", "cloudleaf-types", "--release", "--locked", "--offline",
            "-j2", "--target-dir", TARGET, "--message-format=json",
        ],
        deadline_seconds=350,
        sample=False,
    )
    compiled = [json.loads(line) for line in (out / "cargo/stdout.txt").read_text(encoding="utf-8").splitlines()]
    ring, = {
        file
        for row in compiled if row["reason"] == "compiler-artifact" and row["target"]["name"] == "ring"
        for file in row["filenames"] if file.endswith(".rlib")
    }
    release = TARGET / "release"
    args = [
        "rustc", "--edition", "2024", "-O", "-C", "lto=thin",
        "-C", "codegen-units=1", "-L", f"dependency={release / 'deps'}",
    ]
    for path in (release / "build").glob("ring-*/out"):
        args += ["-L", f"native={path}"]
    for library in ("cloudleaf_input", "cloudleaf_search", "cloudleaf_types"):
        args += ["--extern", f"{library}={release / f'lib{library}.rlib'}"]
    args += ["--extern", f"ring={ring}"]
    source = {
        file.relative_to(ROOT).as_posix(): identity(file)["sha256"]
        for directory in ("cloudleaf-types", "cloudleaf-pinyin", "cloudleaf-input", "cloudleaf-search")
        for file in (ROOT / "crates" / directory / "src").rglob("*.rs")
    }
    binaries = {}
    for name, entry in [
        ("input", "round26_input_probe.rs"),
        ("graph", "round27_graph_diagnostics.rs"),
    ]:
        source_path = ROOT / "tools/performance" / entry
        binary = out / f"round27-{name}.exe"
        linked = run(out / f"link-{name}", [*args, source_path, "-o", binary], sample=False)
        binaries[name] = {"tool": identity(source_path), "binary": identity(binary), "execution": linked}
    assert the_head == head()
    put_json(out / "summary.json", {"head": the_head, "source": source,
                                     "build": compilation, "binaries": binaries})


def read_replay(file):
    rows = file.read_text(encoding="utf-8").splitlines()
    return (
        [row.split("\t") for row in rows if row.startswith("EVENT\t")],
        [row.split("\t") for row in rows if row.startswith("ACK\t")],
    )


def metrics(rows):
    return {
        name: percentile([int(row[column]) for row in rows])
        for name, column in (
            ("elapsed_ns", 5), ("allocation_calls", 6), ("allocated_bytes", 7),
            ("live_absolute_peak", 9), ("edges", 11), ("expansions", 12), ("resident", 13),
        )
    } | {"live_growth_bytes": percentile([int(row[9]) - int(row[8]) for row in rows]),
         "graph_cap_reports": sum(row[14] == "Some(GraphEdges)" for row in rows)}


def compare(candidate, out, repetitions):
    out.mkdir(exist_ok=False)
    original, runtime, language, args = resources()
    candidate = Path(get_json(candidate / "summary.json")["binaries"]["input"]["binary"]["path"])
    binaries = {"baseline": original, "candidate": candidate}
    record = {"baseline": BASELINE, "head_at_replay": head(), "tool": identity(__file__),
              "input": runtime, "lexical": language, "runs": {},
              "binaries": {name: identity(path) for name, path in binaries.items()}}
    for count, fixture in FIXTURES.items():
        for scheme in ("full", "abbreviated"):
            captured = {name: [] for name in binaries}
            observations = []
            for iteration in range(repetitions):
                order = list(binaries) if iteration % 2 == 0 else list(reversed(binaries))
                paired = {}
                for label in order:
                    folder = out / f"{count}-{scheme}-{iteration + 1}-{label}"
                    execution = run(folder, [binaries[label], *args, fixture, scheme, "normal"],
                                    deadline_seconds=150)
                    rows, acks = read_replay(folder / "stdout.txt")
                    assert len(rows) == count
                    captured[label].extend(rows)
                    observations.append({"label": label, "iteration": iteration + 1, **execution})
                    paired[label] = rows, [[ack[1], ack[3]] for ack in acks]
                old, new = paired["baseline"], paired["candidate"]
                assert old[1] == new[1], (count, scheme, iteration, "ack")
                differences = []
                for left, right in zip(old[0], new[0], strict=True):
                    assert left[:5] == right[:5]
                    if left[11:17] == right[11:17]:
                        continue
                    assert scheme == "abbreviated" and left[14] == "Some(GraphEdges)", (left[:5], left[11:17], right[11:17])
                    assert left[15:] == right[15:], (left[:5], left[15:], right[15:])
                    differences.append({"case": left[1], "event": int(left[2]),
                                        "old": left[11:15], "new": right[11:15]})
                if iteration == 0:
                    first = differences
                else:
                    assert first == differences
            record["runs"][f"{count}-{scheme}"] = {
                "fixture": identity(fixture), "events_per_version": count * repetitions,
                "changed_cap_accounting": first, "observations": observations,
                "metrics": {label: metrics(rows) for label, rows in captured.items()},
            }
            put_json(out / "summary.json", record)


def diagnose(candidate, out, labels):
    out.mkdir(exist_ok=False)
    _, _, _, args = resources()
    binary = Path(get_json(candidate / "summary.json")["binaries"]["graph"]["binary"]["path"])
    report = {"tool": identity(__file__), "labels": identity(labels), "binary": identity(binary), "runs": {}}
    columns = (
        "kind", "case", "index", "revision", "raw_length", "graph_edges", "limit", "published_before_cap",
        "buffered_before_cap", "dictionary_rows", "duplicate_edges", "boundary_rejected", "viable_edges",
        "expansions", "resident", "wider_edges", "wider_viable_edges", "candidate_size", "wider_candidate_size",
        "improved_representative", "first_rejected_start", "same_top", "target", "wider_target",
        "target_rank", "wider_target_rank", "search_budget", "wider_graph_budget", "wider_search_budget",
    )
    for count, fixture in FIXTURES.items():
        folder = out / str(count)
        execution = run(folder, [binary, *args, fixture, labels], deadline_seconds=240)
        rows = (folder / "stdout.txt").read_text(encoding="utf-8").splitlines()
        assert rows[-1] == f"DONE\t{count}"
        published = [dict(zip(columns, row.split("\t"), strict=True)) for row in rows[:-1]]
        for entry in published:
            assert int(entry["published_before_cap"]) + int(entry["buffered_before_cap"]) == int(entry["graph_edges"]) == 16384
            assert int(entry["dictionary_rows"]) == int(entry["boundary_rejected"]) + int(entry["duplicate_edges"]) + 16384 + 1
        report["runs"][str(count)] = {"trace": identity(fixture), "events": count,
                                     "execution": execution, "truncated": published}
        put_json(out / "summary.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    builder = commands.add_parser("build")
    builder.add_argument("build_dir", type=Path)
    paired = commands.add_parser("compare")
    paired.add_argument("build_dir", type=Path)
    paired.add_argument("out", type=Path)
    paired.add_argument("--repetitions", type=int, default=2)
    diagnostic = commands.add_parser("diagnose")
    diagnostic.add_argument("build_dir", type=Path)
    diagnostic.add_argument("out", type=Path)
    diagnostic.add_argument("labels", type=Path)
    args = parser.parse_args()
    if args.command == "build":
        build(args.build_dir.resolve())
    elif args.command == "compare":
        if args.repetitions < 1:
            parser.error("--repetitions 至少为 1")
        compare(args.build_dir.resolve(), args.out.resolve(), args.repetitions)
    else:
        diagnose(args.build_dir.resolve(), args.out.resolve(), args.labels.resolve())


if __name__ == "__main__":
    main()

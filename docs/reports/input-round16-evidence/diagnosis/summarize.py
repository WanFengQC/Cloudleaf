from pathlib import Path
import collections
import datetime
import hashlib
import json
import subprocess

root = Path(__file__).resolve().parent
repo = Path(r"C:\Users\EDY\.codex\worktrees\input-round14\Cloudleaf")
groups = collections.defaultdict(list)
lexical_source = repo / ".local/round15-language/signal.tsv"
lexical_counts = {fields[1]: int(fields[2]) for line in lexical_source.read_text(encoding="utf-8").splitlines() if line.startswith("<lexical>\t") for fields in [line.split("\t")]}
cases = []
targets = []
candidate_lines = []
current = None
for line in (root / "diagnosis.stdout.tsv").read_text(encoding="utf-8-sig").splitlines():
    fields = line.split("\t")
    if fields[0] == "CASE":
        cases.append(line)
    elif fields[0] == "TARGET":
        targets.append(line)
    elif fields[0] == "CAND":
        candidate_lines.append(line)
    elif fields[0] == "PATH":
        current = {"structural": int(fields[5].split("=")[1]), "lexical": int(fields[6].split("=")[1]), "edges": []}
        groups[tuple(fields[1:4])].append(current)
    elif fields[0] == "EDGE":
        current["edges"].append({"start": int(fields[3]), "end": int(fields[4]), "text": json.loads(fields[5]), "id": fields[6], "match": fields[7], "lexical": fields[8], "raw_research_field2_sum": lexical_counts.get(fields[6])})

summary = {
    "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "scope": "Round 15 original ranking diagnostic, no training or scoring changes",
    "cases": cases,
    "targets": targets,
    "top10": candidate_lines,
    "groups": [{
        "input": key[0], "ranker": key[1], "target": json.loads(key[2]), "paths": len(paths),
        "best_path": sorted(paths, key=lambda p: (-p["structural"], -p["lexical"], [edge["id"] for edge in p["edges"]]))[0],
    } for key, paths in groups.items()],
    "unique_related_entries": {edge["id"]: edge for paths in groups.values() for path in paths for edge in path["edges"]},
    "commands": [
        {"command": "cargo run --release --locked --offline -- runtime.tsv signal.tsv", "returncode": 101, "reason": "new independent temporary project had no Cargo.lock", "stderr": "diagnosis.stderr.txt"},
        {"command": "cargo generate-lockfile --offline", "returncode": 0},
        {"command": "cargo run --release --locked --offline -- C:\\Users\\EDY\\.codex\\worktrees\\cl009-data\\Cloudleaf\\.local\\round5-clean-6d3d4e1\\release-a\\runtime.tsv C:\\Users\\EDY\\.codex\\worktrees\\input-round14\\Cloudleaf\\.local\\round15-language\\signal.tsv", "returncode": 0, "stdout": "diagnosis.stdout.tsv", "stderr": "diagnosis.stderr-run.txt"},
    ],
    "files": {},
    "source_files": {},
}
for name in ["Cargo.toml", "Cargo.lock", "src/main.rs", "summarize.py", "diagnosis.stdout.tsv", "diagnosis.stderr.txt", "diagnosis.stderr-run.txt", "target/release/r16-old-diagnosis.exe"]:
    file = root / name
    summary["files"][name] = {"bytes": file.stat().st_size, "sha256": hashlib.sha256(file.read_bytes()).hexdigest()}
for name in ["crates/cloudleaf-search/src/lib.rs", "crates/cloudleaf-search/src/search.rs", "crates/cloudleaf-search/src/model.rs", "crates/cloudleaf-search/src/lattice.rs", "crates/cloudleaf-search/src/lexicon.rs", "crates/cloudleaf-pinyin/src/lib.rs"]:
    file = repo / name
    data = file.read_bytes()
    original = subprocess.run(["git", "-C", str(repo), "show", f"5045e18981dadefd3881a05fd021b31712ac243e:{name}"], check=True, stdout=subprocess.PIPE).stdout
    summary["source_files"][name] = {"sha256": hashlib.sha256(data).hexdigest(), "same_as_round15_final": data == original}
(root / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"groups": [{"input": g["input"], "ranker": g["ranker"], "target": g["target"], "paths": g["paths"], "best_structural": g["best_path"]["structural"], "best_lexical": g["best_path"]["lexical"], "segmentation": [e["text"] for e in g["best_path"]["edges"]]} for g in summary["groups"]], "all_production_sources_same_as_round15": all(f["same_as_round15_final"] for f in summary["source_files"].values())}, ensure_ascii=False, indent=2))

"""重放 Round 23 冻结样本的逐例阶段诊断，原文只写入 Git 外目录。"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / ".local"
R23 = "ff09385ebcca323b5f68051ee9bb56c0070696de"
POOL = 50
TASKS = LOCAL / "round23-word-context"
OLD = {
    "base": {"structural": "structural", "character": "character"},
    "supplement": {"character": "supplement"},
}


def file_id(path):
    with path.open("rb") as src:
        fingerprint = hashlib.file_digest(src, "sha256").hexdigest()
    return {"sha256": fingerprint, "bytes": path.stat().st_size}


def save(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")


def load(path):
    return json.loads(path.read_bytes())


def old_pools(mode):
    path = LOCAL / f"round23-exact-language-{mode}/rank/stdout.txt"
    found = {}
    with path.open(encoding="utf-8") as lines:
        for line in lines:
            key, ident, *row = line.rstrip("\r\n").split("\t")
            if not ident.startswith("r23frozen-r23frozen"):
                continue
            ident = ident.removeprefix("r23frozen-")
            if key == "CASE":
                found[ident] = {"target": row[2], "ranks": {}}
            elif key == "POOL":
                size, hit, budget, complete = row
                found[ident]["pool"] = (int(size), hit == "yes", budget, complete == "true")
            elif key == "CAND" and row[2] == found[ident]["target"]:
                found[ident]["ranks"][row[0]] = int(row[1])
    return found, file_id(path)


def frozen_cases():
    def read(edition):
        folder = TASKS / f"frozen-{edition}"
        path = folder / "tasks.jsonl"
        identity = file_id(path)
        if identity["sha256"] != load(folder / "receipt.json")["tasks_sha256"]:
            raise ValueError("冻结样本 SHA-256 不匹配")
        return [json.loads(line) for line in path.read_bytes().splitlines()], identity

    base, base_id = read("base")
    supplement, supp_id = read("supplement")
    if len(base) != 40 or len(supplement) != 40:
        raise ValueError("历史研究分母变化")
    cases = {}
    for past, alternate in zip(base, supplement, strict=True):
        ident = past["case"]
        target = past["source_text"]
        expected = past["source_text_sha256"]
        if (ident in cases or ident != alternate["case"]
                or expected != alternate["source_text_sha256"]
                or target != alternate["source_text"]
                or expected != hashlib.sha256(target.encode()).hexdigest()
                or alternate["mapping_status"] != "ready"
                or past["mapping_status"] not in {"ready", "unmappable"}):
            raise ValueError("冻结样本配对或原分母无效")
        raw = alternate["derived_pinyin"]
        if not raw or (past["mapping_status"] == "ready" and past["derived_pinyin"] != raw):
            raise ValueError("不同原始按键不能合并为词典对照")
        cases[ident] = {"case": ident, "original": past["mapping_status"], "target_sha256": expected,
                        "target": target, "raw": raw, "raw_sha256": hashlib.sha256(raw.encode()).hexdigest()}
    return cases, (base_id, supp_id)


def classify(row, pool):
    if row["surface_prefix"] < row["target_chars"]:
        return "LexiconMissing"
    if row.get("phonetic_error") or row.get("graph_error"):
        return "Unknown"
    if row["paths"] == 0:
        if row["graph_budget"] != "None":
            return "GraphIncomplete"
        if not row["legal"] or row["interpretation_budget"] != "None":
            return "Unknown"
        if not row["compatible_flat_reading"]:
            return "PronunciationMissing"
        if not row["compatible_full_arc_reading"]:
            return "SegmentationMissing"
        return "Unknown"
    if not pool:
        return "Unknown"
    if pool["target_rank"] is None:
        if pool["budget"] in {"Some(Expansions)", "Some(ResidentPaths)"}:
            return "SearchBudgetExceeded"
        if pool["complete"] and pool["size"] == POOL:
            return "PoolRecallLoss"
        return "Unknown"
    if pool["complete"] and pool["word_rank"] > pool["finite-character_rank"]:
        return "RankingError"
    return "Retained"


def optional_rank(raw):
    matched = re.fullmatch(r"Some\(([1-9][0-9]*)\)", raw)
    if matched:
        return int(matched.group(1))
    if raw != "None":
        raise ValueError("未知池名次")
    return None


def parsed(path, cases):
    observed = {}
    with path.open(encoding="utf-8") as src:
        for line in src:
            key, ident, *columns = line.rstrip("\r\n").split("\t")
            if key == "SOURCE":
                continue
            row = observed.setdefault(ident, {"case": ident, "pools": {}})
            if key == "CASE":
                origin, raw, target = columns
                item = cases[ident]
                if raw != item["raw"] or target != item["target"]:
                    raise ValueError("诊断器改变固定输入")
                row.update(input_origin=origin, original_mapping_status=item["original"],
                           target_sha256=item["target_sha256"], raw_sha256=item["raw_sha256"])
            elif key == "SURFACE":
                n, end, gaps = columns
                row.update(target_chars=int(n), surface_prefix=int(end), single_character_gaps=json.loads(gaps))
            elif key == "PHONETIC":
                legal, flat, exact, budget, arcs, work = columns
                row.update(legal=legal == "true", compatible_flat_reading=flat == "true",
                           compatible_full_arc_reading=exact == "true", interpretation_budget=budget,
                           phonetic_arcs=int(arcs), phonetic_work=int(work))
            elif key == "LATTICE":
                budget, n, count, edge_indices, entry_ids = columns
                row.update(graph_budget=budget, graph_edges=int(n), paths=int(count),
                           witness_edges=json.loads(edge_indices), witness_entries=json.loads(entry_ids))
            elif key in {"INPUT_ERROR", "GRAPH_ERROR"}:
                row["phonetic_error" if key == "INPUT_ERROR" else "graph_error"] = True
            elif key == "POOL":
                mode, size, budget, done, expansions, resident, pruned, rank = columns
                row["pools"][mode] = {"size": int(size), "budget": budget, "complete": done == "true",
                                      "expansions": int(expansions), "resident_peak": int(resident),
                                      "beam_pruned": int(pruned), "target_rank": optional_rank(rank)}
            elif key == "REPRESENTATIVE":
                mode, ids, edges, _ = columns
                row["pools"][mode].update(representative_entries=json.loads(ids),
                                          representative_edges=json.loads(edges))
            elif key == "ORDER":
                mode, scorer, rank, breakdown = columns
                row["pools"][mode][f"{scorer}_rank"] = int(rank)
                row["pools"][mode][f"{scorer}_score"] = breakdown
            else:
                raise ValueError("未知诊断事件")
    if set(observed) != set(cases):
        raise ValueError("缺失诊断案例")
    return observed


def verify_historical(rows, edition):
    sources = {}
    for mode, previous in OLD[edition].items():
        old, sources[mode] = old_pools(previous)
        if set(rows) != set(old):
            raise ValueError("历史输出与诊断分母不一致")
        for ident, row in rows.items():
            pool = row["pools"].get(mode)
            old_case = old[ident]
            if "pool" not in old_case:
                if row["original_mapping_status"] != "unmappable" or edition != "base":
                    raise ValueError(f"{ident} 缺失历史候选池")
                continue
            if not pool or (pool["size"], pool["target_rank"] is not None, pool["budget"],
                            pool["complete"]) != old_case["pool"]:
                raise ValueError(f"{ident} 候选池与历史证据不一致")
            for scorer in ("word", "mixed", "finite-character"):
                if pool.get(f"{scorer}_rank") != old_case["ranks"].get(scorer):
                    raise ValueError(f"{ident} {scorer} 排名与历史证据不一致")
            pool["legacy_witten_top10_rank"] = old_case["ranks"].get("character-witten")
    return sources


def summarize(rows, edition):
    groups = {}
    for mode in ("structural", "character"):
        counts = Counter()
        for row in rows.values():
            pool = row["pools"].get(mode)
            diagnosis = classify(row, pool)
            row.setdefault("diagnosis", {})[mode] = diagnosis
            counts[diagnosis] += 1
        groups[mode] = dict(counts)
    return {"dedup_exclusion_demonstrated": 0, "state_merge_or_pruning_loss_demonstrated": 0,
            "multiple_target_paths": sum(row.get("paths", 0) > 1 for row in rows.values()),
            "legal_full_pinyin": sum(row.get("legal", False) for row in rows.values()),
            "matching_full_lexicon_reading": sum(row.get("compatible_full_arc_reading", False)
                                                for row in rows.values()),
            "reachable": sum(row.get("paths", 0) > 0 for row in rows.values()),
            "single_character_gap_positions": sum(len(row["single_character_gaps"])
                                                  for row in rows.values()),
            "has_archived_same_mode": list(OLD[edition]), "diagnoses": groups}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(LOCAL):
        raise ValueError("原始材料必须保存在 Git 外 .local")
    out.mkdir(parents=True, exist_ok=False)
    cases, task_ids = frozen_cases()
    base = load(LOCAL / "round22-search-fixed/summary.json")
    prior_base = load(LOCAL / "round23-exact-language-structural/summary.json")
    prior_supp = load(LOCAL / "round23-exact-language-supplement/summary.json")
    supplement = ROOT.parents[1] / "input-round14/Cloudleaf/.local/round17-supplement-a"
    configurations = {
        "base": (base["runtime"], prior_base["word"], base["models"]["witten"]),
        "supplement": ({"path": supplement / "combined-runtime.tsv", "sha256": prior_supp["sources"][0][0]},
                       prior_supp["word"], {"path": supplement / "model-1.tsv",
                        "sha256": "ecdfb593da055e2065440f36be054bef85dea055909acc2862899d971de30189"}),
    }
    sources, findings, summary = {}, {}, {}
    for edition, resources in configurations.items():
        group = out / edition
        group.mkdir()
        tsv = group / "cases.tsv"
        tsv.write_text("".join(
            "\t".join((row["case"], row["raw"],
                       "supplement-counterfactual" if edition == "base" and row["original"] == "unmappable"
                       else "historical", row["target"])) + "\n" for row in cases.values()
        ), encoding="utf-8", newline="\n")
        command = [str(args.binary.resolve())]
        for item in resources:
            command += [str(item["path"]), item["sha256"]]
        command.append(str(tsv))
        stdout, stderr = group / "stdout.txt", group / "stderr.txt"
        with stdout.open("xb") as out_stream, stderr.open("xb") as err_stream:
            result = subprocess.run(command, stdout=out_stream, stderr=err_stream, check=False, timeout=120)
        save(group / "command.json", {"argv": command, "returncode": result.returncode,
                                      "binary": file_id(args.binary), "cases": file_id(tsv),
                                      "stdout": file_id(stdout), "stderr": file_id(stderr)})
        result.check_returncode()
        rows = parsed(stdout, cases)
        sources[edition] = {"archive": verify_historical(rows, edition), "stdout": file_id(stdout),
                            "stderr": file_id(stderr), "runtime": file_id(Path(resources[0]["path"])),
                            "word_model": file_id(Path(resources[1]["path"])),
                            "witten_model": file_id(Path(resources[2]["path"]))}
        summary[edition] = summarize(rows, edition)
        findings[edition] = list(rows.values())
    record = {"schema": "cloudleaf-round24-frozen-recall-diagnosis-v1", "round23_commit": R23,
              "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
              "binary": file_id(args.binary), "tool": file_id(Path(__file__)), "input_tasks": task_ids,
              "historical_cases": len(cases), "original_base_unmappable": sum(
                  case["original"] == "unmappable" for case in cases.values()),
              "interpretation": "词典派生全拼对照，历史研发，非独立人工 gold",
              "summary": summary, "sources": sources, "cases": findings}
    save(out / "diagnosis.json", record)
    print(json.dumps(summary, ensure_ascii=False))


if __name__ == "__main__":
    main()

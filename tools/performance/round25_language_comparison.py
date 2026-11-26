"""比较固定字符候选、旧词重排、新置信重排和旧上下文束。"""
import argparse
from collections import Counter
from pathlib import Path
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / ".local"
sys.path.insert(0, str(LOCAL))
from r21_evidence_common import get_json, head, identity, percentile, put_json, run

MODES = ["character", "word", "supported", "beam"]


def freeze_cases(folders, out):
    sources = []
    with out.open("x", encoding="utf-8", newline="\n") as stream:
        stream.write("id\tscheme\traw\texpected\n")
        for folder in folders:
            receipt = get_json(folder / "receipt.json")
            path = folder / "tasks.jsonl"
            stamp = identity(path)
            if stamp["sha256"] != receipt["tasks_sha256"]:
                raise ValueError("样本源或冻结身份变化")
            rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
            if len(rows) != receipt["frozen_slots"]:
                raise ValueError("冻结集合分母变化")
            for row in rows:
                status = row["mapping_status"]
                kind = "full" if status == "ready" else status
                raw = row["derived_pinyin"] if kind == "full" else ""
                stream.write("\t".join([row["case"], kind, raw, row["source_text"]]) + "\n")
            sources.append(stamp)
    return sources


def parse(path):
    rows = {}
    model = None
    for line in path.read_text(encoding="utf-8").splitlines():
        kind, *value = line.split("\t")
        if kind == "MODEL":
            corpus, word_bytes, pairs, character_bytes = value
            model = {"corpus_sha256": corpus, "word_bytes": int(word_bytes),
                     "word_pairs": int(pairs), "character_bytes": int(character_bytes)}
            continue
        label, *fields = value
        if kind == "CASE":
            scheme, reach, *work = fields
            rows[label] = {"scheme": scheme, "reach": reach, "paths": [], "beam": [],
                           "edges": int(work[0]) if work else None,
                           "lattice_ns": int(work[1]) if work else None, "runs": {}}
            continue
        row = rows[label]
        if kind == "RUN":
            mode, ns, size, budget, expansions, resident, exact = fields
            row["runs"][mode] = {"ns": int(ns), "size": int(size), "budget": budget,
                                 "expansions": int(expansions), "resident": int(resident),
                                 "complete": exact == "true"}
        elif kind == "PATH":
            a, b, c, hit, *features, ids, path_edges = fields
            names = ("char", "unigram", "conditional", "boundary", "observed", "unknown", "original_final",
                     "positive_gain", "supported_transitions", "bonus", "characters", "structure")
            assert len(features) == len(names)
            row["paths"].append({"character": int(a), "supported": int(b), "word": int(c),
                                 "match": hit == "true", "ids": ids, "edges": path_edges,
                                 **{name: int(v) for name, v in zip(names, features, strict=True)}})
        elif kind == "BEAM":
            rank, hit, char_units, gain, ids, path_edges = fields
            row["beam"].append({"beam": int(rank), "match": hit == "true", "ids": ids, "edges": path_edges,
                                "char": int(char_units), "positive_gain": int(gain)})
        else:
            raise ValueError("未知评估行")
    assert model is not None
    return model, rows


def target(row, mode):
    return next((entry for entry in row["beam" if mode == "beam" else "paths"] if entry["match"]), {})


def cause(target, winner):
    return {
        "likely_more_unknown_tokens": target["unknown"] > winner["unknown"],
        "target_character_better": target["char"] > winner["char"],
        "target_word_mean_worse": target["conditional"] < winner["conditional"],
        "target_unigram_worse": target["unigram"] < winner["unigram"],
        "target_boundary_sum_worse": target["boundary"] < winner["boundary"],
        "target_has_more_tokens": target["observed"] + target["unknown"] > winner["observed"] + winner["unknown"],
        "target_has_more_characters": target["characters"] > winner["characters"],
    }


def summarize(rows):
    result = {"cases": len(rows), "reachability": dict(Counter(x["reach"] for x in rows.values())),
              "unsupported": dict(Counter(x["scheme"] for x in rows.values() if x["scheme"] != "full")),
              "edges": percentile([r["edges"] for r in rows.values() if r["edges"] is not None]),
              "metrics": {}, "word_rank_regression": Counter(), "path_examples": {}}
    base_ranks = {k: target(v, "character").get("character") for k, v in rows.items()}
    for mode in MODES:
        ranks = {k: target(v, mode).get(mode) for k, v in rows.items()}
        distributions = [r["runs"][mode] for r in rows.values() if mode in r["runs"]]
        counter = dict(Counter(x["budget"] for x in distributions))
        strict_ranks = {k: (rank if rows[k]["runs"][mode]["budget"] == "none" else None)
                        for k, rank in ranks.items() if rank is not None}
        result["metrics"][mode] = {
            "top": {f"k{k}": sum(1 <= (v or 51) <= k for v in ranks.values())
                    for k in (1, 3, 5, 10, 50)},
            "without_exhausted_top": {f"k{k}": sum(1 <= (v or 51) <= k for v in strict_ranks.values())
                                      for k in (1, 3, 5, 10, 50)},
            "mrr_at_50": sum(1 / v for v in ranks.values() if v is not None) / len(rows),
            "new_hits": [k for k, v in ranks.items() if v is not None and base_ranks[k] is None],
            "lost_hits": [k for k, v in ranks.items() if v is None and base_ranks[k] is not None],
            "improved_rank": [k for k, v in ranks.items() if (v or 51) < (base_ranks[k] or 51)],
            "regressed_rank": [k for k, v in ranks.items() if (v or 51) > (base_ranks[k] or 51)],
            "no_candidates": sum(x["size"] == 0 for x in distributions),
            "budgets": counter, "executed_queries": len(distributions),
            "latency_ns": percentile([x["ns"] for x in distributions]),
            "expansions": percentile([x["expansions"] for x in distributions]),
            "resident_paths": percentile([x["resident"] for x in distributions]),
        }
    for key, row in rows.items():
        expected = target(row, "character")
        beam = target(row, "beam")
        if expected and beam:
            result["word_rank_regression"]["both_saved"] += 1
            if beam["ids"] != expected["ids"] or beam["edges"] != expected["edges"]:
                result["word_rank_regression"]["different_path_representative"] += 1
        if expected and expected["word"] > expected["character"]:
            new_first = next(x for x in row["paths"] if x["word"] == 1)
            reason = cause(expected, new_first)
            result["word_rank_regression"]["cases_target_dropped_in_rank"] += 1
            if expected["character"] == 1:
                result["word_rank_regression"]["character_winner_dropped"] += 1
            result["word_rank_regression"].update(name for name, yes in reason.items() if yes)
            result["path_examples"][key] = {"target": expected, "word_winner": new_first,
                                             "beam_target": beam, "comparison": reason}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--round25-freeze", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    fixture = output / "cases.tsv"
    families = freeze_cases([LOCAL / "round23-word-context/frozen-supplement",
                             LOCAL / "round24-corpus/frozen-v2", args.round25_freeze], fixture)
    bound = identity(fixture)
    fixed_base = get_json(LOCAL / "round22-search-fixed/summary.json")
    fixed_supplement = ROOT.parents[1] / "input-round14/Cloudleaf/.local/round17-supplement-a"
    base = Path(fixed_base["runtime"]["path"])
    supplement = fixed_supplement / "combined-runtime.tsv"
    base_model = Path(fixed_base["models"]["witten"]["path"])
    supplement_model = fixed_supplement / "model-1.tsv"
    model_lock = {
        "base": (base, fixed_base["runtime"]["sha256"], base_model, fixed_base["models"]["witten"]["sha256"],
                 ["final-minimum-base", "final-probable-base-a"]),
        "supplement": (supplement, "c9ab8b56b9d960091a1c39c37b7c1df7882ff50de2a64767174af8f4244c219f",
                       supplement_model, "ecdfb593da055e2065440f36be054bef85dea055909acc2862899d971de30189",
                       ["final-minimum-supplement-a", "final-probable-supplement-a"]),
    }
    result = {"head": head(), "binary": identity(args.binary), "recipe": identity(__file__),
              "scoring_source": identity(ROOT / "crates/cloudleaf-search/src/word_context.rs"),
              "probe_source": identity(ROOT / "crates/cloudleaf-input/examples/language_audit.rs"),
              "fixture": bound, "sources": families, "formal": {"top1": None, "top5": None, "mrr": None},
              "locks": {}, "runs": {}}
    for lex_name, (lex, lex_sha, chars, char_sha, word_models) in model_lock.items():
        for mode, folder in zip(("minimum", "probability"), word_models, strict=True):
            tag = f"{lex_name}-{mode}"
            word = LOCAL / "round24-corpus" / folder / "word-context.tsv"
            word_identity = identity(word)
            assert identity(lex)["sha256"] == lex_sha and identity(chars)["sha256"] == char_sha
            lock = get_json(word.with_name("receipt.json"))
            assert lock["lexicon_sha256"] == lex_sha and lock["model"]["sha256"] == word_identity["sha256"]
            result["locks"][tag] = {"lexicon": lex_sha, "character": char_sha, "word": word_identity}
    put_json(output / "before-outcomes.json", result)
    for lex_name, (lex, lex_sha, chars, char_sha, word_models) in model_lock.items():
        for mode, folder in zip(("minimum", "probability"), word_models, strict=True):
            tag = f"{lex_name}-{mode}"
            word = LOCAL / "round24-corpus" / folder / "word-context.tsv"
            word_sha = result["locks"][tag]["word"]["sha256"]
            execution = run(output / tag, [args.binary.resolve(), lex, lex_sha, word, word_sha,
                                           chars, char_sha, fixture, bound["sha256"]], deadline_seconds=300)
            built, rows = parse(output / tag / "stdout.txt")
            assert len(rows) == 120
            cases = {group: {k: v for k, v in rows.items() if re.match(f"r{group}frozen[0-9]+$", k)}
                     for group in (23, 24, 25)}
            assert all(len(v) == 40 for v in cases.values())
            result["runs"][tag] = {"execution": execution, "model_bytes": built,
                                   "groups": {k: summarize(v) for k, v in cases.items()}}
    put_json(output / "summary.json", result)


if __name__ == "__main__":
    main()

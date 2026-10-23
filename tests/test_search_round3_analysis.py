"""复核报告语义；此处候选仅为机制fixture，不充当语言正确答案。"""
import copy
import importlib.util
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("round3_analysis", ROOT / "scripts/analyze_search_round3.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def candidate(text="甲", identity="a"):
    return {"text": text, "score": -100, "ids": [identity], "features": {"words": 1, "prefixes": 0},
            "records": [{"id": identity, "source": "fixture", "source_record_id": "1", "license_id": "MIT"}]}


def case(identity="a", samples=None):
    samples = [1] * 30 if samples is None else samples
    return {"id": identity, "family": "fixture", "input": "shi", "kind": "short", "status": "ok",
            "guaranteed": True, "waiting": False, "fallback_preserved": True, "whole_entry_complete": True,
            "candidates": [candidate()], "samples_ns": samples,
            **{key: value for key, value in MODULE.quantiles(samples).items() if key != "samples"},
            "statistics": {"graph_edges": 3, "expansions": 4, "resident_high_water": 2, "beam_pruned": 0, "budget": None}}


def report():
    runs = {}
    for label, values in MODULE.CONFIGURATIONS.items():
        method, width, edges, expansions, resident = values
        runs[label] = {"iterations_per_case": 30, "warmup_per_case": 5, "cases": [case()],
                       "peak_process_memory_bytes": 1024, "memory_method": "fixture", "load_ns": 10,
                       "lexicon_entries": 1, "trie_nodes": 2,
                       "configuration": {"method": method, "beam_width": width, "max_edges": edges,
                                         "max_expansions": expansions, "max_resident_paths": resident,
                                         "max_input_bytes": 512, "max_letters": 256, "output_k": 5, "ranker": "Structural"}}
    return {"schema_version": 1, "experiment": "cloudleaf-real-lexicon-resource-v1", "language_accuracy_valid": False,
            **{field: "0" * 64 for field in ("source_sha256", "binary_sha256", "cases_sha256", "protocol_sha256", "evidence_sha256")},
            "runs": runs}


class ComparisonTests(unittest.TestCase):
    def test_same_text_with_changed_score_ids_features_or_records_is_failure(self):
        baseline = case()
        for field, value in [("score", -99), ("ids", ["different"]), ("features", {"words": 2}),
                             ("records", [{"license_id": "unknown"}])]:
            changed = copy.deepcopy(baseline)
            changed["candidates"][0][field] = value
            result = MODULE.compare_rows(baseline, changed, True)
            self.assertFalse(result["full_equivalence"])
            self.assertFalse(result["rank_checks"][0]["equal_fields"][field])
            self.assertEqual(result["lost"], [])

    def test_rank_order_is_not_disguised_by_equal_candidate_set(self):
        baseline = case()
        baseline["candidates"].append(candidate("乙", "b"))
        changed = copy.deepcopy(baseline)
        changed["candidates"].reverse()
        result = MODULE.compare_rows(baseline, changed, True)
        self.assertFalse(result["full_equivalence"])
        self.assertEqual(len(result["rank_changes"]), 2)
        self.assertTrue(all(not row["equal"] for row in result["rank_checks"]))

    def test_every_incomplete_gate_blocks_full_verdict_even_equal_outputs(self):
        baseline = case()
        changes = [{"guaranteed": False}, {"status": "rejected"}, {"whole_entry_complete": False},
                   {"statistics": {**baseline["statistics"], "budget": "Expansions"}},
                   {"statistics": {**baseline["statistics"], "beam_pruned": 1}}]
        for patch in changes:
            partial = {**copy.deepcopy(baseline), **patch}
            for left, right in [(partial, partial), (partial, baseline), (baseline, partial)]:
                self.assertIsNone(MODULE.compare_rows(left, right, True)["full_equivalence"])

    def test_beam_loss_and_pruning_remain_observable_with_null_verdict(self):
        baseline = case()
        baseline["candidates"].append(candidate("乙", "b"))
        beam = copy.deepcopy(baseline)
        beam["candidates"].pop()
        beam["guaranteed"] = False
        beam["statistics"]["beam_pruned"] = 2
        result = MODULE.compare_rows(baseline, beam, True)
        self.assertIsNone(result["full_equivalence"])
        self.assertEqual(result["lost"], ["乙"])
        self.assertIn("beam_pruned", result["candidate_incomplete_reasons"])

    def test_status_waiting_fallback_change_is_not_lost(self):
        for field in ("waiting", "fallback_preserved"):
            baseline, changed = case(), case()
            changed[field] = not changed[field]
            self.assertFalse(MODULE.compare_rows(baseline, changed, True)["full_equivalence"])

    def test_complete_exact_mismatch_fails_but_partial_exact_does_not_pass(self):
        result = report()
        result["runs"]["exact"]["cases"][0]["candidates"][0]["score"] -= 1
        analysis = MODULE.analyze_report(result, True, {"a": "shi"})
        self.assertEqual(analysis["failures"], ["reference/exact/a"])
        result["runs"]["reference"]["cases"][0]["guaranteed"] = False
        result["runs"]["exact"]["cases"][0]["guaranteed"] = False
        analysis = MODULE.analyze_report(result, True, {"a": "shi"})
        self.assertEqual(analysis["failures"], [])
        self.assertIsNone(analysis["secondary_exact_position"][0]["full_equivalence"])


class ResourceTests(unittest.TestCase):
    def test_pooled_quantiles_are_distinct_from_max_single_case_quantile(self):
        rows = [case("large", [0] * 100), case("small", [100])]
        summary = MODULE.group_summary(rows)
        self.assertEqual(summary["pooled_samples"]["p95_ns"], 0)
        self.assertEqual(summary["max_case_quantiles"]["p95_ns"], 100)
        self.assertEqual(summary["pooled_samples"]["samples"], 101)

    def test_rejected_budget_empty_and_beam_remain_in_group_denominators(self):
        rows = [case(str(index)) for index in range(5)]
        rows[0]["status"] = "rejected"
        rows[0]["candidates"] = []
        rows[1]["statistics"]["budget"] = "GraphEdges"
        rows[1]["guaranteed"] = False
        rows[2]["candidates"] = []
        rows[3]["statistics"]["beam_pruned"] = 3
        rows[3]["guaranteed"] = False
        run = report()["runs"]["reference"]
        run["cases"] = rows
        result = MODULE.groups(run, {row["id"]: "shi" for row in rows})
        self.assertEqual(result["all"]["cases"], 5)
        self.assertEqual(result["all"]["rejected"], 1)
        self.assertEqual(result["all"]["budget_reasons"], {"GraphEdges": 1})
        self.assertEqual(sum(group["cases"] for group in result["outcome"].values()), 5)
        self.assertFalse(result["process_memory"]["per_case_available"])

    def test_letter_length_keeps_unicode_and_boundaries(self):
        self.assertEqual(MODULE.length_group("lü"), "1-3")
        self.assertEqual(MODULE.length_group("xi'an"), "4-6")
        self.assertEqual(MODULE.length_group(""), "0")

    def test_budget_time_ratio_is_not_called_complete_search_speedup(self):
        old, new = case(), case()
        old["guaranteed"] = False
        old["statistics"]["budget"] = "Expansions"
        result = MODULE.paired_row(old, new)
        self.assertIsNone(result["p95_ratio_if_both_complete"])


class ValidationTests(unittest.TestCase):
    def test_missing_duplicate_or_changed_case_is_rejected(self):
        for rows in [[], [case(), case()], [case("unexpected")]]:
            item = report()
            item["runs"]["exact"]["cases"] = rows
            with self.assertRaises(ValueError):
                MODULE.validate_report(item, True)

    def test_configuration_fingerprint_sample_and_language_flags_are_strict(self):
        variants = []
        item = report()
        item["binary_sha256"] = "not-a-sha"
        variants.append(item)
        item = report()
        item["runs"]["position"]["configuration"]["output_k"] = 3
        variants.append(item)
        item = report()
        item["runs"]["position"]["cases"][0]["samples_ns"] = [1]
        variants.append(item)
        item = report()
        item["runs"]["position"]["cases"][0]["p95_ns"] = 0
        variants.append(item)
        item = report()
        item["top1"] = 1.0
        variants.append(item)
        for item in variants:
            with self.assertRaises(ValueError):
                MODULE.validate_report(item, True)

    def test_historical_runtime_and_protocol_mismatch_refuse_pairing(self):
        original = {"cases_sha256": "0" * 64, "protocol_sha256": "0" * 64,
                    "evidence": {"runtime": {"sha256": "0" * 64}}}
        for field in ("cases_sha256", "protocol_sha256", "runtime"):
            changed = copy.deepcopy(original)
            if field == "runtime":
                changed["evidence"]["runtime"]["sha256"] = "1" * 64
            else:
                changed[field] = "1" * 64
            with self.assertRaises(ValueError):
                MODULE.history_analysis(original, changed)


if __name__ == "__main__":
    unittest.main()

"""验证真实数据实验的证据门禁与差分分母；临时伪证据只用于格式测试，不代表已获许可。"""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("real_evaluation", ROOT / "scripts/evaluate_real_lexicon.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class EvidenceTests(unittest.TestCase):
    def evidence(self, folder):
        runtime = folder / "runtime.tsv"
        runtime.write_text("# cloudleaf-lexicon-v2\n" + "\t".join(MODULE.HEADER) + "\n"
                           "fixture:1\tzhong guo\t中国\t\tunknown\t\tfixture\t1\tMIT\n", encoding="utf-8", newline="\n")
        document = folder / "review.txt"
        document.write_text("仅为测试批准书与许可证格式的伪造文档，不能用于正式导入。", encoding="utf-8")
        item = {"path": document.name, "sha256": MODULE.sha256(document)}
        evidence = {"schema_version": 1, "dataset_id": "fixture-not-product", "real_data": True,
                    "runtime": {"path": runtime.name, "sha256": MODULE.sha256(runtime)}, "build_report": item,
                    "sources": [{"id": "fixture", "version": "1", "url": "https://example.invalid/fixture",
                                 "license_id": "MIT", "license": item, "audit": item,
                                 "review_status": "approved", "synthetic": False}]}
        path = folder / "evidence.json"
        path.write_text(json.dumps(evidence), encoding="utf-8")
        return path, evidence, runtime

    def test_evidence_checks_every_file_and_retains_unknown(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            path, _, runtime = self.evidence(folder)
            loaded, _, counts = MODULE.validate_evidence(path)
            self.assertEqual(loaded, runtime)
            self.assertEqual(counts["unknown_frequencies"], 1)
            self.assertEqual(counts["multi_character_entries"], 1)
            runtime.write_text(runtime.read_text(encoding="utf-8") + "坏行\n", encoding="utf-8", newline="\n")
            with self.assertRaises(ValueError):
                MODULE.validate_evidence(path)

    def test_rejects_synthetic_unapproved_unmapped_and_zero_filled_data(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            path, evidence, runtime = self.evidence(folder)
            for field, value in [("synthetic", True), ("review_status", "pending"), ("license_id", "unknown")]:
                changed = copy.deepcopy(evidence)
                changed["sources"][0][field] = value
                path.write_text(json.dumps(changed), encoding="utf-8")
                with self.assertRaises(ValueError):
                    MODULE.validate_evidence(path)
            raw = runtime.read_text(encoding="utf-8")
            for changed in [raw.replace("\t\tunknown", "\t0\tunknown"), raw.replace("\tunknown", "\tmanual"),
                            raw.replace("\tfixture\t1\t", "\tunregistered\t1\t")]:
                runtime.write_text(changed, encoding="utf-8", newline="\n")
                evidence["runtime"]["sha256"] = MODULE.sha256(runtime)
                path.write_text(json.dumps(evidence), encoding="utf-8")
                with self.assertRaises(ValueError):
                    MODULE.validate_evidence(path)

    def test_resource_probe_has_no_gold_or_train_test_claim(self):
        cases = MODULE.validate_cases()
        self.assertTrue(any(row["input"] == "" for row in cases))
        self.assertTrue(any(row["kind"] == "invalid" for row in cases))
        self.assertTrue(any(row["kind"] == "unmatched" for row in cases))
        self.assertTrue(all("gold" not in row and "split" not in row for row in cases))


class ComparisonTests(unittest.TestCase):
    def case(self, name="a", candidates=None, guaranteed=True):
        return {"id": name, "status": "ok", "candidates": candidates or [], "guaranteed": guaranteed,
                "whole_entry_candidates": [], "fallback_preserved": True, "waiting": False,
                "statistics": {"budget": None, "beam_pruned": 0}}

    def test_reports_added_lost_and_ranking_separately(self):
        delta = MODULE.delta(["甲", "乙", "丙"], ["乙", "甲", "丁"])
        self.assertEqual(delta["added"], ["丁"])
        self.assertEqual(delta["lost"], ["丙"])
        self.assertEqual(len(delta["rank_changes"]), 2)

    def test_complete_reference_difference_fails_but_budget_is_not_oracle(self):
        baseline = self.case(candidates=[{"text": "甲", "score": -100}])
        changed = self.case(candidates=[{"text": "乙", "score": -100}])
        runs = {"reference": {"cases": [baseline]}, "exact": {"cases": [changed]}}
        cases = [{"id": "a"}]
        _, failures = MODULE.compare(runs, cases)
        self.assertEqual(failures, ["a"])
        baseline["guaranteed"] = False
        comparisons, failures = MODULE.compare(runs, cases)
        self.assertEqual(failures, [])
        self.assertIsNone(comparisons[0]["exact_reference_equal"])

    def test_missing_or_duplicate_case_never_disappears_from_denominator(self):
        runs = {"reference": {"cases": [self.case()]}, "exact": {"cases": []}}
        with self.assertRaises(ValueError):
            MODULE.compare(runs, [{"id": "a"}])
        runs["exact"]["cases"] = [self.case(), self.case()]
        with self.assertRaises(ValueError):
            MODULE.compare(runs, [{"id": "a"}])

    def test_truncated_whole_entry_delta_is_never_claimed_valid(self):
        value = self.case(candidates=[{"text": "甲"}])
        value["whole_entry_complete"] = False
        runs = {"reference": {"cases": [copy.deepcopy(value)]}, "exact": {"cases": [value]}}
        comparisons, _ = MODULE.compare(runs, [{"id": "a"}])
        whole = comparisons[0]["whole_entry_vs_exact"]
        self.assertFalse(whole["comparison_valid"])
        self.assertFalse(whole["whole_entry_complete"])
        self.assertEqual(whole["observed_delta"]["added"], ["甲"])


class ActualProbeTests(unittest.TestCase):
    def test_release_host_replays_full_paths_unknown_and_rejected_inputs(self):
        # 此处仅为MIT机制fixture，不经过真实数据批准门禁，不冒充正式词条实验。
        binary = MODULE.build_host()
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            runtime, cases = folder / "runtime.tsv", folder / "cases.tsv"
            runtime.write_text("# cloudleaf-lexicon-v2\n" + "\t".join(MODULE.HEADER) + "\n"
                               "fixture:1\tzhong\t中\t\tunknown\t\tfixture\t1\tMIT\n"
                               "fixture:2\tguo\t国\t\tunknown\t\tfixture\t2\tMIT\n", encoding="utf-8", newline="\n")
            cases.write_text("# cloudleaf-search-resource-cases-v1\nid\tfamily\tinput\tkind\n"
                             "whole\tf1\tzhongguo\tpath\nunknown\tf2\tzzzz\tunknown\n"
                             "bad\tf3\tni,hao\tinvalid\n", encoding="utf-8", newline="\n")
            results = []
            for method in ["reference", "exact", "beam"]:
                command = [str(binary), str(runtime), str(cases), method, "64", "3", "1", "16384", "200000", "50000", "5"]
                result, _, _ = MODULE.measure(command)
                self.assertFalse(result["language_accuracy_valid"])
                self.assertFalse(result["frequency_feature_enabled"])
                self.assertEqual(result["missing_frequencies"], 2)
                self.assertEqual(len(result["cases"]), 3)
                whole, unknown, bad = result["cases"]
                self.assertEqual(whole["candidates"][0]["text"], "中国")
                self.assertEqual(whole["whole_entry_candidates"], [])
                self.assertEqual(whole["candidates"][0]["features"]["frequency_units"], 0)
                self.assertEqual(len(whole["candidates"][0]["records"]), 2)
                self.assertEqual(len(whole["samples_ns"]), 3)
                self.assertEqual(unknown["candidates"], [])
                self.assertTrue(unknown["fallback_preserved"])
                self.assertEqual(bad["status"], "rejected")
                results.append(whole["candidates"])
            self.assertEqual(results[0], results[1])
            self.assertEqual(results[1], results[2])


if __name__ == "__main__":
    unittest.main()

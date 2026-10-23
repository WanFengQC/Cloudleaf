"""验证机械评估隔离和差分门禁；不把构造数据当作语言质量。"""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("evaluate_search", ROOT / "scripts/evaluate_search.py")
EVALUATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EVALUATOR)


class SearchEvaluationTests(unittest.TestCase):
    def test_checked_in_mechanical_families_are_disjoint(self):
        result = EVALUATOR.validate_isolation()
        self.assertEqual(result["case_counts"], {"dev": 2, "test": 11})
        self.assertFalse(result["language_accuracy_valid"])
        self.assertFalse(result["old_83_seed_used_for_training_or_tuning"])

    def test_leaking_family_and_duplicate_id_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder)
            for path in (ROOT / "tests/search-mechanics-v1").glob("*.tsv"):
                (target / path.name).write_bytes(path.read_bytes())
            cases = target / "cases.tsv"
            original = cases.read_text(encoding="utf-8")
            cases.write_text(original.replace("boundary-development", "transition-training"), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "家族交叉"):
                EVALUATOR.validate_isolation(target)
            cases.write_text(original.replace("dev-umlaut", "dev-boundary"), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "重复ID"):
                EVALUATOR.validate_isolation(target)

    def test_differential_gate_keeps_failures_in_denominator(self):
        runs = {}
        for ranker in ("structural", "weighted", "bigram", "context"):
            for method in ("reference", "exact", "beam64", "beam1"):
                runs[f"{method}-{ranker}"] = {"cases": [{"id": "case", "status": "ok", "candidates": []}]}
        self.assertEqual(EVALUATOR.compare_results(runs)["exact-structural"]["total_cases"], 1)
        changed = json.loads(json.dumps(runs))
        changed["exact-weighted"]["cases"][0]["status"] = "rejected"
        with self.assertRaisesRegex(ValueError, "精确搜索"):
            EVALUATOR.compare_results(changed)


if __name__ == "__main__":
    unittest.main()

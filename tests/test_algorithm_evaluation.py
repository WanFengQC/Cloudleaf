"""评估器防泄漏与指标分母测试；不以待测排序生成期望。"""

import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("cloudleaf_evaluation", ROOT / "scripts/evaluate_algorithms.py")
EVALUATION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EVALUATION)


def example(identifier="one", split="dev", group="one", input_text="hao", context="-", gold="好"):
    return {"id": identifier, "split": split, "category": "common", "group": group,
            "input": input_text, "context": context, "gold": gold, "correction": "clean", "note": "手写测试"}


def result(rank, candidates, required=False, known=True):
    return {"gold_rank": rank, "candidates": candidates, "correction": "required" if required else "clean",
            "correction_attempt": False, "correction_correct": False, "durations_ns": [100, 200],
            "lexicon_known": known, "error": "-"}


class EvaluationIntegrityTests(unittest.TestCase):
    def test_historical_input_cannot_be_renamed_or_recased_into_heldout(self):
        excluded = EVALUATION.historical_inputs(ROOT / "tests/fixtures/input-v1.tsv")
        self.assertIn("nihao", excluded)
        for text in ("NiHaO", "nhao", "nihaox", "Hello"):
            with self.subTest(text=text), self.assertRaisesRegex(ValueError, "历史事件"):
                EVALUATION.validate_rows([example("new-id", "heldout", input_text=text)], excluded)

    def test_family_cannot_cross_dev_and_heldout_even_with_different_inputs(self):
        with self.assertRaisesRegex(ValueError, "group不能跨"):
            EVALUATION.validate_rows([example(), example("two", "heldout", "one", "ha")], set())

    def test_equivalent_multigold_order_does_not_bypass_duplicate_check(self):
        with self.assertRaisesRegex(ValueError, "gold重复"):
            EVALUATION.validate_rows([example(gold="好|号"), example("two", gold="号|好")], set())

    def test_distinct_context_intents_can_share_input_inside_one_split(self):
        EVALUATION.validate_rows([example(), example("two", context="编号", gold="号")], set())

    def test_all_misses_oov_and_unsupported_stay_in_denominator(self):
        metrics = EVALUATION.aggregate([result(1, ["好"]), result(3, ["是", "时", "事"]),
                                        result(0, [], required=True), result(0, [], known=False)])
        self.assertEqual(metrics["count"], 4)
        self.assertEqual(metrics["top1"], 0.25)
        self.assertEqual(metrics["top5"], 0.5)
        self.assertAlmostEqual(metrics["mrr"], (1 + 1 / 3) / 4)
        self.assertEqual(metrics["candidate_availability"], 0.5)
        self.assertEqual(metrics["gold_coverage"], 0.5)
        self.assertEqual(metrics["latency"]["samples"], 8)

    def test_no_correction_predictions_have_undefined_precision_and_zero_recall(self):
        metrics = EVALUATION.aggregate([result(0, [], required=True), result(1, ["好"])])["correction"]
        self.assertFalse(metrics["capability"])
        self.assertEqual(metrics["attempts"], 0)
        self.assertIsNone(metrics["precision"])
        self.assertEqual(metrics["recall"], 0)
        self.assertIsNone(EVALUATION.aggregate([result(1, ["好"])])["correction"]["recall"])

    def test_percentiles_use_all_samples_and_nearest_rank(self):
        self.assertEqual(EVALUATION.percentile(list(range(1, 101)), 95), 95)
        self.assertEqual(EVALUATION.percentile([9, 1, 2], 50), 2)
        self.assertIsNone(EVALUATION.percentile([], 99))

    def test_clean_input_modification_is_a_prediction_but_never_correction_success(self):
        clean = result(1, ["好"])
        clean["correction_attempt"] = True
        clean["correction_correct"] = True
        required = result(1, ["国"], required=True)
        required["correction_attempt"] = True
        required["correction_correct"] = True
        metrics = EVALUATION.aggregate([clean, required])["correction"]
        self.assertEqual(metrics["attempts"], 2)
        self.assertEqual(metrics["correct"], 1)
        self.assertEqual(metrics["precision"], 0.5)
        self.assertEqual(metrics["recall"], 1)

    def test_actual_seed_is_group_isolated_and_does_not_reuse_regression_inputs(self):
        rows = EVALUATION.read_dataset(ROOT / "tests/evaluation/algorithm-v1.tsv", ROOT / "tests/fixtures/input-v1.tsv")
        self.assertGreaterEqual(len(rows), 80)
        self.assertEqual(sum(row["split"] == "dev" for row in rows), 16)
        self.assertGreaterEqual(sum(row["split"] == "heldout" for row in rows), 64)


if __name__ == "__main__":
    unittest.main()

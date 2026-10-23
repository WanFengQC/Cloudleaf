"""独立朴素穷举的结构语义测试；fixture文本不作为语言gold。"""
import copy
import importlib.util
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("restricted_oracle", ROOT / "scripts/evaluate_restricted_oracle.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def entry(identity, pinyin, text):
    return {"id": identity, "pinyin": pinyin, "text": text, "frequency": "", "frequency_kind": "unknown",
            "annotation": "", "source": "fixture", "source_record_id": identity, "license_id": "MIT"}


class OracleTests(unittest.TestCase):
    def test_complete_multiple_splits_deduplicate_by_best_score_and_stable_ids(self):
        rows = [entry("03", "a na", "甲乙"), entry("01", "a", "甲"), entry("02", "na", "乙"), entry("04", "an", "丙")]
        result = MODULE.exhaustive(rows, "ana")
        self.assertTrue(result["guaranteed"])
        self.assertGreater(result["enumerated_complete_paths"], 1)
        self.assertEqual(result["candidates"][0]["text"], "甲乙")
        self.assertEqual(result["candidates"][0]["ids"], ["03"])
        self.assertEqual(result["candidates"][0]["features"]["missing_frequencies"], 1)
        self.assertEqual(result["candidates"][0]["records"][0]["source"], "fixture")
        tied = MODULE.exhaustive([entry("02", "an", "同文"), entry("01", "an", "同文")], "an")
        self.assertEqual(tied["candidates"][0]["ids"], ["01"])

    def test_apostrophe_unicode_prefix_waiting_unknown_and_invalid_preserve_denominator(self):
        rows = [entry("a", "xi an", "分段"), entry("b", "xian", "整音"), entry("c", "lv", "吕")]
        self.assertEqual([row["text"] for row in MODULE.exhaustive(rows, "xi'an")["candidates"]], ["分段"])
        self.assertEqual(MODULE.exhaustive(rows, "lÜ")["candidates"][0]["text"], "吕")
        prefix = MODULE.exhaustive(rows, "xia")
        self.assertTrue(all(row["features"]["prefixes"] == 1 for row in prefix["candidates"]))
        self.assertTrue(MODULE.exhaustive(rows, "xi'")["waiting"])
        self.assertEqual(MODULE.exhaustive(rows, "zzzz")["candidates"], [])
        self.assertEqual(MODULE.exhaustive(rows, "")["candidates"], [])
        for value in ("'xi", "xi''an", "ni,hao", "a" * 257):
            self.assertEqual(MODULE.exhaustive(rows, value)["status"], "rejected")

    def test_independent_expansion_or_edge_exhaustion_cannot_claim_equivalence(self):
        rows = [entry("a", "a", "甲"), entry("b", "a", "乙")]
        full = MODULE.exhaustive(rows, "aaa")
        for partial in (MODULE.exhaustive(rows, "aaa", expansion_limit=1), MODULE.exhaustive(rows, "aaa", edge_limit=1)):
            self.assertFalse(partial["guaranteed"])
            self.assertIsNone(MODULE.compare_rows(partial, full, True)["full_equivalence"])

    def test_selection_is_fixed_reading_and_ids_not_rust_output_or_row_order(self):
        rows = [entry("3", "a", "丙"), entry("1", "a", "甲"), entry("2", "a", "乙"), entry("0", "not-selected", "丁")]
        selected, counts = MODULE.select_rows(rows)
        self.assertEqual([row["id"] for row in selected], ["1", "2"])
        self.assertEqual(counts["a"]["available"], 3)
        self.assertEqual(counts["lv"]["selected_ids"], [])
        self.assertEqual(MODULE.select_rows(list(reversed(rows)))[0], selected)
        changed = copy.deepcopy(rows)
        changed[0]["frequency_kind"] = "manual"
        changed[1]["frequency_kind"] = "manual"
        with self.assertRaises(ValueError):
            MODULE.select_rows(changed)


if __name__ == "__main__":
    unittest.main()

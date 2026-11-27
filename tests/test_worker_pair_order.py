from collections import Counter
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools/performance"))
from round26_worker_isolation import matrix, pair_schedule


class PairOrderTests(unittest.TestCase):
    def test_seeded_matrix_records_six_of_each_order(self):
        stages = pair_schedule(12, 302711)
        self.assertEqual(stages, pair_schedule(12, 302711))
        self.assertEqual(Counter(stages), {"idle": 6, "infer": 6})
        self.assertNotEqual(stages, ["idle", "infer"] * 6)
        runs = list(matrix(["current"], ["normal"], 12, False, True, stages))
        self.assertEqual(len(runs), 24)
        self.assertEqual([run[3] for run in runs[::2]], stages)
        for number, (first, second) in enumerate(zip(runs[::2], runs[1::2], strict=True), 1):
            self.assertEqual(first[:3], (number, "171", "lexical"))
            self.assertEqual(first[:3], second[:3])
            self.assertEqual(first[4:], ("normal", "current"))
            self.assertEqual(first[4:], second[4:])
            self.assertEqual({first[3], second[3]}, {"idle", "infer"})

    def test_previous_order_without_seed(self):
        runs = list(matrix(["current"], ["normal"], 8, False, True))
        self.assertEqual([run[3] for run in runs[::2]], ["idle", "infer"] * 4)


if __name__ == "__main__":
    unittest.main()

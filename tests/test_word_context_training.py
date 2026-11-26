"""词语统计实际实现的隔离、分词和计数回归。"""

from collections import Counter
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

import build_word_context as context
import freeze_word_context_validation as freeze


def row(text, family, sentence_id="1"):
    return {"sentence_id": sentence_id, "family": family, "fragment_index": 0,
            "text": text, "sha256": context.sha(text.encode("utf-8"))}


def body(*rows):
    return "".join(json.dumps(value, ensure_ascii=False) + "\n" for value in rows).encode("utf-8")


def family(train):
    return next(f for i in range(1, 100) if (
        int(context.sha((context.SPLIT_SALT + (f := f"cmn:{i}")).encode())[:16], 16) % 100 < 80
    ) == train)


class WordContextTrainingTests(unittest.TestCase):
    def test_uncovered_overlap_requires_global_segmentation(self):
        terms = {"甲乙", "甲", "乙丙"}
        self.assertEqual(list(context.segment("甲乙丙", terms, [2, 1])), ["甲", "乙丙"])
        self.assertEqual(list(context.segment("甲乙丁", terms, [2, 1])), ["甲乙", context.UNKNOWN])

    def test_deduplicates_training_and_preserves_cross_source_denominator(self):
        same = row("甲乙丙丁", family(True))
        members = context.overlap_index([("foreign", "甲乙戊己庚辛壬癸子丑")])
        duplicate = row(same["text"], same["family"], "2")
        foreign = row("甲乙戊己庚辛壬癸子丑寅", family(True))
        nontrain = row("甲丁庚", family(True))
        records, counts = context.load_train(body(same, duplicate, foreign, nontrain),
                                             {nontrain["sha256"]}, members)
        self.assertEqual(records, [same])
        self.assertEqual(counts["source_fragments"], 4)
        self.assertEqual(counts["dropped_exact_train_duplicate"], 1)
        self.assertEqual(counts["blocked_external_development_overlap"], 1)
        self.assertEqual(counts["blocked_exact_nontrain_hash"], 1)

    def test_rejects_dev_family_and_mutated_input_identity(self):
        with self.assertRaisesRegex(ValueError, "非训练"):
            context.load_train(body(row("甲乙丙丁", family(False))), set(),
                               context.overlap_index([]))
        wrong = row("甲乙丙丁", family(True))
        wrong["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "身份"):
            context.load_train(body(wrong), set(), context.overlap_index([]))

    def test_counts_with_boundaries_and_utf8_sorted_format_are_reproducible(self):
        records = [row("甲乙", family(True)), row("甲丙", family(True))]
        counts, pairs, stats = context.train(records, {"甲": 1, "乙": 1})
        self.assertEqual(counts, Counter({context.BOS: 2, "甲": 2, "乙": 1,
                                        context.UNKNOWN: 1, context.EOS: 2}))
        self.assertEqual(stats["fully_covered_segments"], 1)
        self.assertEqual(stats["bigrams_observed"], 6)
        self.assertEqual(pairs[(context.BOS, "甲")], 2)
        self.assertEqual(pairs[(context.UNKNOWN, context.EOS)], 1)
        data = context.encode("a" * 64, "b" * 64, counts, pairs)
        other = context.train(list(reversed(records)), {"甲": 1, "乙": 1})
        self.assertEqual(data, context.encode("a" * 64, "b" * 64, *other[:2]))
        self.assertEqual(data.decode().splitlines()[0], "# cloudleaf-word-context-v1")
        self.assertNotIn(b"\r", data)

    def test_derived_pinyin_uses_stable_ids_after_minimum_lexical_words(self):
        readings = {"甲乙": [("a", "jiayi")], "甲": [("z", "jia")],
                    "乙丙": [("j", "yibing"), ("a", "yipin")]}
        self.assertEqual(freeze.reading("甲乙丙", readings, [1, 2]), "jiayipin")
        self.assertIsNone(freeze.reading("丙甲", readings, [1, 2]))

    def test_external_development_never_accepts_formal_heldout_or_other_source(self):
        item = {"case": "1", "source_family": "wikimedia-wikinews-zh", "formal_gold": False,
                "split": "dev", "source_text": "甲乙", "source_text_sha256": context.sha("甲乙".encode())}
        self.assertTrue(context.leaks("甲，乙", context.protected_development([item])))
        for bad in ({"split": "heldout"}, {"formal_gold": True}, {"source_family": "unknown"}):
            with self.assertRaisesRegex(ValueError, "用途"):
                context.protected_development([{**item, **bad}])


if __name__ == "__main__":
    unittest.main()

"""自编夹具只验证格式与边界，真实数据质量另报。"""

from pathlib import Path
import gzip
import json
import tempfile
import unittest

from tools.data_pipeline.pipeline import build, canonical, digest, read_json, verify_release, write_json
from tools.data_pipeline.pipeline import rejection_categories, runtime_annotation
from tools.data_pipeline.prefix_snapshot import extract_prefix
from tools.data_pipeline.word_source import normalize_word_reading


class WordSourceTests(unittest.TestCase):
    def test_explicit_mandarin_segmentation_preserves_boundaries_and_rejects_ambiguity(self):
        for raw, expected in [("shǒuyè", ["shou", "ye"]), ("nǐhǎo", ["ni", "hao"]),
                              ("cháng'ān", ["chang", "an"]), ("gēge", ["ge", "ge"]), ("lǜsè", ["lv", "se"])]:
            self.assertEqual(normalize_word_reading(raw), expected)
        for raw in ["sau² jip⁶", "ㄕㄡˇ", "ni3hao3", "shou/ye", "biáng", "a''i", " hǎo"]:
            with self.assertRaises(ValueError):
                normalize_word_reading(raw)

    def test_real_word_schema_preserves_languages_pos_english_and_unknown_frequency(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, license_path, lock = root / "source.gz", root / "LICENSE", root / "lock.json"
            license_path.write_bytes(b"fixture-only license\n")
            row = {"word": "首頁", "lang_code": "zh", "pos": "noun", "pos_title": "名詞",
                   "senses": [{"glosses": ["網站的主要頁面"], "examples": [{"text": "不輸出引用"}]}],
                   "sounds": [{"zh_pron": "shǒuyè", "tags": ["Mandarin", "Pinyin"]},
                              {"zh_pron": "sau² jip⁶", "tags": ["Cantonese", "Jyutping"]}],
                   "translations": [{"lang_code": "en", "word": "homepage", "sense": "網站頁面"}]}
            with gzip.open(source, "wb") as stream:
                stream.write(canonical(row))
            write_json(lock, {"schema_version": 1, "review_status": "approved", "format": "wiktionary-zh-gzip",
                             "source_id": "cloudleaf:test-words", "version": "fixture-v1", "license_id": "MIT",
                             "sha256": digest(source.read_bytes()), "license_sha256": digest(license_path.read_bytes()),
                             "url": "https://example.invalid/source", "limitations": ["僅夾具"],
                             "attribution": "Cloudleaf fixture", "license_url": "https://example.invalid/license",
                             "transformations": "test", "automated_download_allowed": False})
            output = root / "release"
            build(lock, source, license_path, output)
            verify_release(output)
            record = json.loads((output / "records.jsonl").read_text(encoding="utf-8"))
            self.assertEqual(record["schema_version"], 3)
            self.assertEqual(record["pos"], "noun")
            self.assertEqual(record["gloss_language"], "zh")
            self.assertIsNone(record["frequency"])
            self.assertEqual(record["source_fields"]["english_translations"][0]["word"], "homepage")
            self.assertNotIn("examples", record["source_fields"]["senses"][0])
            self.assertEqual(read_json(output / "quality.json")["counts"]["missing_english_gloss"], 0)
            renamed = root / "another-local-name.gz"
            renamed.write_bytes(source.read_bytes())
            second = root / "another-release"
            build(lock, renamed, license_path, second)
            self.assertEqual({p.name: p.read_bytes() for p in output.iterdir()},
                             {p.name: p.read_bytes() for p in second.iterdir()})
            projection = (output / "runtime.tsv").read_text(encoding="utf-8").splitlines()[2].split("\t")
            self.assertEqual(projection[5], "homepage")
            self.assertNotIn("網站的主要頁面", projection[5])
            record["source_fields"]["english_translations"] = []
            self.assertEqual(runtime_annotation(record), "")

    def test_research_prefix_keeps_only_complete_lines_and_locks_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            raw = b'{"word":"one"}\n' + b'{"word":"two"}\n'
            complete = gzip.compress(raw, mtime=0)
            # 去掉gzip尾部校验，显式标为未完整原件的研究前缀。
            prefix = root / "prefix.partial"
            prefix.write_bytes(complete[:-8])
            first, second = root / "first.jsonl", root / "second.jsonl"
            a = extract_prefix(prefix, digest(prefix.read_bytes()), first)
            b = extract_prefix(prefix, digest(prefix.read_bytes()), second)
            self.assertEqual(first.read_bytes(), raw)
            self.assertEqual(a, b)
            self.assertTrue(a["research_only"])
            self.assertFalse(a["full_gzip_crc_verified"])
            self.assertIsNone(a["original_sha256"])
            self.assertEqual(a["line_count"], 2)
            with self.assertRaisesRegex(ValueError, "哈希"):
                extract_prefix(prefix, "0" * 64, root / "bad.jsonl")
            self.assertFalse((root / "bad.jsonl").exists())
            prefix.write_bytes(complete)
            with self.assertRaisesRegex(ValueError, "完整gzip"):
                extract_prefix(prefix, digest(prefix.read_bytes()), root / "complete.jsonl")

    def test_runtime_and_rights_isolations_are_not_source_malformations(self):
        reasons = ["unsupported_reading", "unsupported_word_reading", "ambiguous_or_unsupported_word_reading",
                   "word_reading_budget", "unreviewed_additional_rights_marker", "rights_or_quotation_marker", "field_count"]
        counts = rejection_categories([{"reason": reason} for reason in reasons])
        self.assertEqual(counts["unsupported_runtime_readings"], 2)
        self.assertEqual(counts["ambiguous_runtime_readings"], 1)
        self.assertEqual(counts["runtime_budget_isolations"], 1)
        self.assertEqual(counts["rights_or_quotation_isolations"], 2)
        self.assertEqual(counts["malformed_or_conflicting_records"], 1)

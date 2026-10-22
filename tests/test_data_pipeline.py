"""夹具仅验证管线行为，不作为正式词典或语言准确率样本。"""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from zipfile import ZipFile

from tools.data_pipeline.pipeline import (
    HEADER, SYLLABLES, activate, atomic_json, build, canonical, digest, fetch,
    normalize_reading, publish_lock, read_json, rollback, runtime, verify_release, write_json,
)


class PipelineTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source.zip"
        self.license = self.root / "license.txt"
        self.license.write_text("测试许可，仅用于测试夹具\n", encoding="utf-8", newline="\n")
        self.lock = self.root / "lock.json"

    def prepare(self, content: str, *, name: str = "Unihan_Readings.txt", extra: dict | None = None) -> None:
        with ZipFile(self.source, "w") as archive:
            archive.writestr(name, content.encode("utf-8"))
        self.lock_data = {"schema_version": 1, "review_status": "approved", "format": "unihan-zip",
                          "source_id": "cloudleaf:test-source", "license_id": "MIT", "version": "test-1",
                          "url": "https://example.invalid/source.zip", "automated_download_allowed": False,
                          "sha256": digest(self.source.read_bytes()), "license_sha256": digest(self.license.read_bytes()),
                          "limitations": ["仅单元测试"], "attribution": "Cloudleaf test fixture",
                          "transformations": "test-only", "license_url": "https://example.invalid/license"}
        if extra:
            self.lock_data.update(extra)
        write_json(self.lock, self.lock_data)

    def create(self, name: str = "release") -> Path:
        output = self.root / name
        build(self.lock, self.source, self.license, output)
        return output

    def test_nullable_frequency_and_rich_evidence(self) -> None:
        self.prepare("# 夹具\nU+4E00\tkMandarin\tyī\nU+4E00\tkDefinition\tone\n")
        output = self.create()
        row = json.loads((output / "records.jsonl").read_text(encoding="utf-8"))
        self.assertIsNone(row["frequency"])
        self.assertIsNone(row["pos"])
        self.assertEqual(row["raw_reading"], "yī")
        self.assertEqual(row["raw_senses"], ["one"])
        self.assertEqual(row["evidence"][0]["line"], 2)
        runtime = (output / "runtime.tsv").read_text(encoding="utf-8").splitlines()
        self.assertEqual(runtime[1], HEADER)
        self.assertEqual(runtime[2].split("\t")[3:5], ["", "unknown"])

    def test_deterministic_build_and_offline(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\nU+4E8C\tkMandarin\tèr\n")
        with patch("urllib.request.urlopen", side_effect=AssertionError("构建不得联网")):
            first, second = self.create("first"), self.create("second")
        self.assertEqual({p.name: p.read_bytes() for p in first.iterdir()},
                         {p.name: p.read_bytes() for p in second.iterdir()})

    def test_hash_mismatch_fails_without_output(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        self.source.write_bytes(self.source.read_bytes() + b"changed")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            self.create()
        self.assertFalse((self.root / "release").exists())

    def test_license_and_review_fail_closed(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        self.license.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "许可证"):
            self.create()
        self.lock_data["review_status"] = "pending"
        write_json(self.lock, self.lock_data)
        with self.assertRaisesRegex(ValueError, "未审核"):
            self.create()

    def test_no_automatic_cedict_or_unapproved_download(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        with patch("urllib.request.urlopen", side_effect=AssertionError("不能请求")):
            with self.assertRaisesRegex(ValueError, "未允许"):
                fetch(self.lock, self.root / "download")

    def test_archive_traversal_rejected(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n", name="../Unihan_Readings.txt")
        with self.assertRaisesRegex(ValueError, "归档成员"):
            self.create()
        self.assertFalse((self.root / "release").exists())

    def test_archive_size_limit_and_utf8(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        with patch("tools.data_pipeline.pipeline.MAX_EXPANDED", 2):
            with self.assertRaisesRegex(ValueError, "展开体积"):
                self.create()
        with ZipFile(self.source, "w") as archive:
            archive.writestr("Unihan_Readings.txt", b"\xff")
        self.lock_data["sha256"] = digest(self.source.read_bytes())
        write_json(self.lock, self.lock_data)
        with self.assertRaises(UnicodeDecodeError):
            self.create()

    def test_conflicts_isolated_duplicate_counted_and_bad_rows_reported(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\nU+4E00\tkMandarin\tyī\nU+4E8C\tkMandarin\tèr\n"
                     "U+4E8C\tkMandarin\tsān\nbad\nU+110000\tkMandarin\tyī\nU+4E09\tkMandarin\txyz\n")
        output = self.create()
        quality = read_json(output / "quality.json")
        self.assertEqual(quality["counts"]["accepted_records"], 1)
        self.assertEqual(quality["counts"]["duplicate_property_lines"], 1)
        self.assertEqual(quality["counts"]["conflicting_characters"], 1)
        self.assertEqual(quality["rejection_reasons"], {"field_count": 1, "invalid_scalar": 1,
                         "source_property_conflict": 1, "unsupported_reading": 1})

    def test_normalization_does_not_guess_readings(self) -> None:
        self.assertEqual([normalize_reading(value) for value in ["lǜ", "nǚ", "jǚ", "hao3", "Lu:4"]],
                         ["lv", "nv", "ju", "hao", "lv"])
        for value in ["ê", "biáng", "hao9", "n", "xi-an", "e\u0302"]:
            with self.assertRaisesRegex(ValueError, "unsupported_reading"):
                normalize_reading(value)

    def test_python_and_rust_syllable_inventory_match(self) -> None:
        rust = Path(__file__).resolve().parents[1] / "crates/cloudleaf-pinyin/src/lib.rs"
        table = rust.read_text(encoding="utf-8").split("const SYLLABLE_GROUPS:", 1)[1].split("];", 1)[0]
        import re
        words = set(" ".join(re.findall(r'"([a-z ]+)"', table)).split())
        self.assertEqual(SYLLABLES, words)

    def test_local_cedict_preserves_raw_senses_and_deduplicates(self) -> None:
        self.prepare("", extra={"format": "cedict-text"})
        line = "你好 你好 [ni3 hao3] /hello/hi/\n"
        self.source.write_text(line + line + "bad\n", encoding="utf-8", newline="\n")
        self.lock_data["sha256"] = digest(self.source.read_bytes())
        write_json(self.lock, self.lock_data)
        output = self.create()
        row = json.loads((output / "records.jsonl").read_text(encoding="utf-8"))
        self.assertEqual(row["raw_senses"], ["hello", "hi"])
        self.assertEqual(row["pinyin"], ["ni", "hao"])
        self.assertEqual(len(row["evidence"]), 2)
        self.assertEqual(read_json(output / "quality.json")["counts"]["duplicate_records"], 1)

    def test_corruption_detected_and_rejected_before_activation(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        output = self.create()
        store = self.root / "store"
        activate(output, store)
        before = (store / "current.json").read_bytes()
        (output / "runtime.tsv").write_bytes(b"corrupted")
        with self.assertRaisesRegex(ValueError, "哈希"):
            activate(output, store)
        self.assertEqual((store / "current.json").read_bytes(), before)

    def test_cross_resource_semantics_verified_even_if_hash_updated(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        output = self.create()
        write_json(output / "index.json", {"schema_version": 2, "by_pinyin": {}, "by_text": {}})
        manifest = read_json(output / "manifest.json")
        data = (output / "index.json").read_bytes()
        manifest["resources"]["index.json"] = {"sha256": digest(data), "bytes": len(data)}
        write_json(output / "manifest.json", manifest)
        with self.assertRaisesRegex(ValueError, "索引不一致"):
            verify_release(output)

    def test_rehashed_facts_projection_and_index_cannot_bypass_semantics(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        attacks = [
            {"text": ""}, {"text": "一\x00"}, {"schema_version": 1}, {"source_version": "wrong"},
            {"id": "entry:forged"}, {"raw_reading": "èr"}, {"raw_senses": ["bad\x01"]},
            {"evidence": [{"member": "Unihan_Readings.txt", "line": -1, "property": "kMandarin", "raw": "yī"}]},
            {"pinyin": ["yi"] * 17}, {"source_record_id": "U4E8C:0"},
        ]
        for number, attack in enumerate(attacks):
            with self.subTest(attack=attack):
                output = self.create(f"attack{number}")
                record = json.loads((output / "records.jsonl").read_text(encoding="utf-8"))
                record.update(attack)
                (output / "records.jsonl").write_bytes(canonical(record))
                # 攻击者同步重建投影、索引和各资源哈希，仍必须被事实层语义校验拒绝。
                if all("\x00" not in value and "\x01" not in value for value in [record["text"], *record["raw_senses"]]):
                    (output / "runtime.tsv").write_bytes(runtime([record]))
                write_json(output / "index.json", {"schema_version": 2,
                           "by_pinyin": {" ".join(record["pinyin"]): [record["id"]]},
                           "by_text": {record["text"]: [record["id"]]}})
                manifest = read_json(output / "manifest.json")
                for name in manifest["resources"]:
                    data = (output / name).read_bytes()
                    manifest["resources"][name] = {"sha256": digest(data), "bytes": len(data)}
                write_json(output / "manifest.json", manifest)
                with self.assertRaises(ValueError):
                    verify_release(output)

    def test_manifest_symlink_rejected(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        output = self.create()
        original = output / "manifest.json"
        external = self.root / "external.json"
        external.write_bytes(original.read_bytes())
        original.unlink()
        try:
            original.symlink_to(external)
        except OSError as error:
            self.skipTest(f"本环境不允许创建符号链接：{error}")
        with self.assertRaisesRegex(ValueError, "链接或越界"):
            verify_release(output)

    def test_atomic_upgrade_and_rollback_preserve_user_data(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        first = self.create("first")
        self.prepare("U+4E8C\tkMandarin\tèr\n")
        second = self.create("second")
        store = self.root / "store"
        store.mkdir()
        user = store / "user-dictionary.json"
        user.write_bytes(b"private-user-data")
        initial = activate(first, store)
        self.assertEqual(activate(first, store), initial)
        upgraded = activate(second, store)
        self.assertEqual(upgraded["previous"], initial["current"])
        restored = rollback(store)
        self.assertEqual(restored["current"], initial["current"])
        self.assertEqual(user.read_bytes(), b"private-user-data")

    def test_damaged_rollback_target_and_replace_failure_keep_pointer(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        first = self.create("first")
        self.prepare("U+4E8C\tkMandarin\tèr\n")
        second = self.create("second")
        store = self.root / "store"
        activate(first, store)
        before = (store / "current.json").read_bytes()
        with patch("tools.data_pipeline.pipeline.os.replace", side_effect=OSError("模拟原子替换失败")):
            with self.assertRaises(OSError):
                activate(second, store)
        self.assertEqual((store / "current.json").read_bytes(), before)
        state = activate(second, store)
        previous = store / "releases" / state["previous"]["release"]
        (previous / "runtime.tsv").write_bytes(b"broken")
        before = (store / "current.json").read_bytes()
        with self.assertRaises(ValueError):
            rollback(store)
        self.assertEqual((store / "current.json").read_bytes(), before)

    def test_concurrent_writer_and_pointer_traversal_fail_closed(self) -> None:
        self.prepare("U+4E00\tkMandarin\tyī\n")
        output = self.create()
        store = self.root / "store"
        with publish_lock(store):
            with self.assertRaisesRegex(ValueError, "已有发布写者"):
                activate(output, store)
        atomic_json(store / "current.json", {"schema_version": 1, "current": {"release": "../outside", "manifest_sha256": "a"}, "previous": None})
        with self.assertRaisesRegex(ValueError, "越界"):
            activate(output, store)


if __name__ == "__main__":
    unittest.main()

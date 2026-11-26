"""研究投影复用真实 bundle 校验器；夹具不代表实际授权。"""
import json
from pathlib import Path
import tempfile
import unittest

from tools.data_pipeline import pipeline as p
from tools.data_pipeline import semantic_bundle as bundle
from tools.data_pipeline.semantic_lookup import export_index
from test_semantic_bundle import fixture, write_content


class SemanticLookupTests(unittest.TestCase):
    def test_projection_preserves_directed_relation_and_does_not_invent_reverse_gloss(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = fixture(root)
            bundle_path = root / "checked"
            verified = bundle.build_bundle(source, bundle_path)
            out = root / "index.json"
            report = export_index(bundle_path, verified["manifest_sha256"], out)
            payload = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(report["explicit_translations"], 1)
            self.assertEqual(report["english_source_glosses"], 0)
            self.assertEqual(payload["scope"], "research_only")
            self.assertEqual(payload["relations"][0]["source_sense_id"], "sense:fixture-one")
            self.assertEqual(payload["relations"][0]["target_sense_id"], "sense:fixture-two")
            self.assertEqual(payload["senses"][0]["definition"], "机械夹具")
            self.assertFalse(payload["senses"][0]["english_source_gloss"])
            other = root / "replay.json"
            export_index(bundle_path, verified["manifest_sha256"], other)
            self.assertEqual(out.read_bytes(), other.read_bytes())

    def test_bad_binding_pending_fact_and_rights_mismatch_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = fixture(root)
            content = p.read_json(source / "content.json")
            content["records"][1]["status"] = "pending"
            write_content(source, content)
            bundle_path = root / "checked"
            verified = bundle.build_bundle(source, bundle_path)
            out = root / "index.json"
            with self.assertRaisesRegex(ValueError, "清单摘要"):
                export_index(bundle_path, "f" * 64, out)
            self.assertFalse(out.exists())
            report = export_index(bundle_path, verified["manifest_sha256"], out)
            self.assertEqual(report["senses"], 1)
            self.assertEqual(report["explicit_translations"], 0)
            rights = p.read_json(source / "rights.json")
            rights["license_id"] = "different-license"
            p.write_json(source / "rights.json", rights)
            content["sources"][0]["rights"] = bundle.descriptor(source, "rights.json")
            write_content(source, content)
            invalid = root / "bad-rights"
            verified = bundle.build_bundle(source, invalid)
            another = root / "must-not-exist.json"
            with self.assertRaisesRegex(ValueError, "来源权利"):
                export_index(invalid, verified["manifest_sha256"], another)
            self.assertFalse(another.exists())

"""真实导出入口的机制测试；临时来源不是产品许可或语言质量证据。"""
import importlib.util
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("translation_dictionary_export", ROOT / "scripts/export_translation_dictionary.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class TranslationDictionaryExportTests(unittest.TestCase):
    def package(self, folder, rows=None, source_status="approved"):
        folder.mkdir()
        files={"LICENSE.txt": b"MIT fixture only; not product data license\n", "NOTICE.txt": b"Fixture authors; not independent language gold\n",
               "index.json":b"{}\n", "quality.json":b"{}\n", "rejected.jsonl":b"", "runtime.tsv":b"fixture-not-runtime-parser-input\n"}
        license_sha=hashlib.sha256(files["LICENSE.txt"]).hexdigest()
        source={"source_id":"fixture-source","version":"fixture-v1","format":"cedict-zip-v1","sha256":"1"*64,
                "license_id":"MIT","license_sha256":license_sha,"review_status":source_status,
                "approved_scope":"Mechanism fixture raw gloss projection; not production approval"}
        if rows is None:
            rows=[{"schema_version":2,"id":"stable-entry","source":"fixture-source","source_version":"fixture-v1",
                   "source_record_id":"raw-row-1","license_id":"MIT","text":"测试🙂","raw_senses":["exact raw gloss🙂","second definition"],
                   "frequency":None,"frequency_kind":"unknown","pos":None}]
        files["records.jsonl"]="".join(json.dumps(row,ensure_ascii=False,sort_keys=True)+"\n" for row in rows).encode("utf-8")
        files["source-lock.json"]=(json.dumps(source,sort_keys=True)+"\n").encode()
        resources={}
        for name, raw in files.items():
            (folder/name).write_bytes(raw)
            resources[name]={"bytes":len(raw),"sha256":hashlib.sha256(raw).hexdigest()}
        manifest={"schema_version":2,"runtime_format":"cloudleaf-lexicon-v2","entry_count":len(rows),"resources":resources,
                  "source":source,"source_lock_sha256":resources["source-lock.json"]["sha256"]}
        raw=(json.dumps(manifest,sort_keys=True)+"\n").encode()
        (folder/"manifest.json").write_bytes(raw)
        return hashlib.sha256(raw).hexdigest(), rows

    def test_missing_package_cannot_be_exported(self):
        with self.assertRaises(ValueError):
            MODULE.export_dictionary(ROOT / ".local/nonexistent-package", "0" * 64, ROOT / ".local/nonexistent-export")

    def test_exact_projection_retains_license_notice_identity_and_unknown_semantics(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            package=root/"package"
            expected, original=self.package(package)
            before={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in package.iterdir()}
            report=MODULE.export_dictionary(package,expected,root/"export")
            value=json.loads((root/"export/catalog.json").read_bytes())
            self.assertEqual(value["entries"][0]["id"],original[0]["id"])
            self.assertEqual(value["entries"][0]["raw_glosses"],original[0]["raw_senses"])
            self.assertEqual(value["entries"][0]["status"],"research_only")
            self.assertEqual(value["mappings"],[])
            self.assertEqual(value["sources"][0]["scope"],["raw_gloss"])
            self.assertFalse(report["production_ready"])
            self.assertFalse(report["human_identity_or_rights_proven"])
            self.assertEqual(report["translation_mappings_created"],0)
            self.assertEqual((root/"export/LICENSE.txt").read_bytes(),(package/"LICENSE.txt").read_bytes())
            self.assertEqual((root/"export/NOTICE.txt").read_bytes(),(package/"NOTICE.txt").read_bytes())
            self.assertEqual((root/"export/SOURCE.json").read_bytes(),(package/"source-lock.json").read_bytes())
            self.assertEqual(before,{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in package.iterdir()})
            self.assertNotIn("frequency",value["entries"][0])

    def test_hash_corruption_and_unapproved_source_fail_before_catalog_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            expected,_=self.package(root/"corrupt")
            (root/"corrupt/LICENSE.txt").write_bytes(b"changed")
            with self.assertRaises(ValueError):
                MODULE.export_dictionary(root/"corrupt",expected,root/"bad-export")
            self.assertFalse((root/"bad-export/catalog.json").exists())
            expected,_=self.package(root/"pending",source_status="pending")
            with self.assertRaises(ValueError):
                MODULE.export_dictionary(root/"pending",expected,root/"pending-export")
            self.assertFalse((root/"pending-export/catalog.json").exists())

    def test_bad_rows_do_not_disappear_from_denominator(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            _,original=self.package(root/"base")
            for index, change in enumerate(["duplicate","wrong-source","boolean-schema","empty-gloss"]):
                rows=copy.deepcopy(original)
                if change=="duplicate": rows.append(copy.deepcopy(rows[0]))
                elif change=="wrong-source": rows[0]["source"]="other"
                elif change=="boolean-schema": rows[0]["schema_version"]=True
                else: rows[0]["raw_senses"]=[]
                package=root/f"package-{index}"
                expected,_=self.package(package,rows)
                output=root/f"output-{index}"
                with self.assertRaises(ValueError):
                    MODULE.export_dictionary(package,expected,output)
                self.assertFalse((output/"catalog.json").exists())

    def test_same_fixed_input_has_identical_catalog_and_never_overwrites_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            expected,_=self.package(root/"package")
            a=MODULE.export_dictionary(root/"package",expected,root/"a")
            b=MODULE.export_dictionary(root/"package",expected,root/"b")
            self.assertEqual(a["catalog_sha256"],b["catalog_sha256"])
            with self.assertRaises(FileExistsError):
                MODULE.export_dictionary(root/"package",expected,root/"a")


if __name__ == "__main__":
    unittest.main()

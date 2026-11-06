from pathlib import Path
import importlib.util
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("supplement", ROOT / "scripts/prepare_input_supplement.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class InputSupplementTests(unittest.TestCase):
    def record(self):
        raw = "漢 汉 [han4] /Han/"
        return {"id":"original", "traditional":"漢", "simplified":"汉", "text":"汉", "pinyin":["han"],
            "source_record_id":MODULE.source_key("漢","汉","han4"),
            "evidence":[{"line":1,"member":"cedict_ts.u8","raw":raw}]}

    def test_projects_only_original_readings_without_invented_frequency(self):
        record = self.record()
        rows, lineage = MODULE.traditional_rows([record],[record["evidence"][0]["raw"]],"fixed",set())
        fields = rows[0].rstrip("\n").split("\t")
        self.assertEqual(fields[1:5],["han","漢","","unknown"])
        self.assertEqual(fields[-2:], [record["source_record_id"],"CC-BY-SA-4.0"])
        self.assertIsNone(lineage[0]["frequency"])
        self.assertFalse(lineage[0]["production_approved"])
        self.assertEqual(lineage[0]["original_record_ids"],["original"])

    def test_existing_pair_is_not_duplicated(self):
        record = self.record()
        self.assertEqual(MODULE.traditional_rows([record],[],"fixed",{("漢","han")}),([],[]))

    def test_source_evidence_change_is_rejected(self):
        record = self.record()
        with self.assertRaises(ValueError):
            MODULE.traditional_rows([record],["漢 汉 [gan4] /changed/"],"fixed",set())

    def test_input_order_does_not_change_ids_or_payload(self):
        record = self.record()
        duplicate = {**record,"id":"duplicate"}
        source = [record["evidence"][0]["raw"]]
        self.assertEqual(MODULE.traditional_rows([record,duplicate],source,"fixed",set()),
                         MODULE.traditional_rows([duplicate,record],source,"fixed",set()))

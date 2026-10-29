"""前置门禁攻击/分母测试；临时许可与人工声明均是格式fixture，不是真实批准。"""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("language_readiness", ROOT / "scripts/check_language_readiness.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ReadinessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.folder = Path(cls.temporary.name)
        def artifact(name, data):
            path = cls.folder / name
            path.write_bytes(data)
            return {"path": name, "sha256": MODULE.digest(path)}
        review = artifact("fixture-review.txt", "仅测试格式，不是真实授权或人工标注证明。".encode("utf-8"))
        runtime = artifact("fixture-runtime.txt", b"not a real lexicon")
        sealed = artifact("fixture-sealed.bin", b"opaque fixture; cryptography not verified by this tool")
        params = artifact("fixture-parameters.json", b'{"fixture_only":true}')
        binary = artifact("fixture-binary.bin", b"not executable")
        cls.rows = []
        quotas = MODULE.family_quotas(13340)
        serial = 0
        for split, count in quotas.items():
            for _ in range(count):
                cls.rows.append({"case_id": f"case-{serial}", "family_id": f"family-{serial}", "source_id": "fixture-eval",
                                 "source_unit_id": f"unit-{serial}", "near_duplicate_group": f"near-{serial}", "split": split,
                                 "normalized_input_sha256": MODULE.normalized_digest(f"fixture-input-{serial}"),
                                 "task_sha256": MODULE.normalized_digest(f"fixture-task-{serial}")})
                serial += 1
        index = artifact("fixture-index.jsonl", ("\n".join(json.dumps(row) for row in cls.rows) + "\n").encode("utf-8"))
        sources = [{"id": key, "role": role, "version": "fixture", "official_url": "https://example.invalid/fixture",
                    "review_status": "approved", "approved_scope": "格式测试而非实际批准", "artifact": data,
                    "permissions": {"evaluation": True, "commercial": False, "modification": False, "redistribution": False},
                    "license": review, "audit": review} for key, role, data in
                   [("fixture-lexicon", "lexicon", runtime), ("fixture-eval", "evaluation-corpus", sealed)]]
        cls.plan = {"schema_version": 1, "experiment": "cloudleaf-formal-language-prerequisites-v1", "registered_at": "2020-01-01T00:00:00Z",
                    "evaluator_id": "fixture-evaluator", "protocol_sha256": MODULE.protocol_digest(),
                    "family_ratios": [70, 15, 15], "minimum_heldout_families": 2000, "normalization": "ascii-lower-umlaut-v1",
                    "sources": sources, "gold": {"status": "independent-human-reviewed", "source_id": "fixture-eval",
                    "dictionary_derived": False, "synthetic": False, "old_seed_used": False, "independent_annotators": True,
                    "annotation_protocol": review, "human_review": review}, "family_index": index,
                    "seal": {"state": "sealed", "gold_opened": False, "custodian_id": "fixture-custodian", "sealed_at": "2020-02-01T00:00:00Z",
                             "family_index_sha256": index["sha256"], "artifact": sealed, "protection": "custodian-encrypted", "custodian_attestation": review},
                    "freeze": {"frozen_at": "2020-03-01T00:00:00Z", "parameters": params, "source_sha256": MODULE.source_sha256(),
                               "protocol_sha256": MODULE.protocol_digest(), "binary": binary, "lexicon": runtime, "lexicon_source_id": "fixture-lexicon"}}

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def changed_index(self, rows):
        path = self.folder / "changed-index.jsonl"
        path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8", newline="\n")
        plan = copy.deepcopy(self.plan)
        plan["family_index"] = {"path": path.name, "sha256": MODULE.digest(path)}
        plan["seal"]["family_index_sha256"] = MODULE.digest(path)
        return plan

    def test_complete_metadata_can_pass_prerequisites_without_reading_gold_or_language_metrics(self):
        result = MODULE.audit(self.plan, self.folder)
        self.assertTrue(result["prerequisites_ready"], result["issues"])
        self.assertFalse(result["language_accuracy_valid"])
        self.assertFalse(result["sealed_gold_opened_by_tool"])
        self.assertIsNone(result["top1"])
        self.assertEqual(result["sections"]["split_isolation"]["details"]["families"]["heldout"], 2001)

    def test_current_checkpoint_missing_real_human_gold_is_rejected(self):
        path = ROOT / "config/evaluation/language-readiness-v1.json"
        result = MODULE.audit(MODULE.read_json(path), path.parent)
        self.assertFalse(result["prerequisites_ready"])
        self.assertTrue(any(item["section"] == "gold" for item in result["issues"]))
        self.assertIsNone(result["mrr"])

    def test_each_cross_split_family_source_near_input_or_task_overlap_is_rejected(self):
        heldout = next(index for index, row in enumerate(self.rows) if row["split"] == "heldout")
        for field in ("family_id", "source_unit_id", "near_duplicate_group", "normalized_input_sha256", "task_sha256"):
            rows = copy.deepcopy(self.rows)
            rows[heldout][field] = rows[0][field]
            with self.assertRaisesRegex(ValueError, "泄漏"):
                MODULE.split_index(self.changed_index(rows), self.folder, {source["id"]: source for source in self.plan["sources"]})

    def test_duplicate_ids_legacy_seed_and_plaintext_gold_cannot_enter_index(self):
        for patch in ({"case_id": self.rows[0]["case_id"]}, {"normalized_input_sha256": MODULE.normalized_digest("nihao")}, {"gold": ["伪造答案"]}):
            rows = copy.deepcopy(self.rows)
            rows[-1].update(patch)
            with self.assertRaises(ValueError):
                MODULE.split_index(self.changed_index(rows), self.folder, {source["id"]: source for source in self.plan["sources"]})

    def test_cross_source_near_duplicate_groups_cannot_cross_any_split(self):
        # 近重复组在整份清单中全局标识，转载不能靠改来源绕过隔离。
        for earlier, later in (("train", "dev"), ("train", "heldout"), ("dev", "heldout")):
            with self.subTest(earlier=earlier, later=later):
                rows = copy.deepcopy(self.rows)
                first = next(row for row in rows if row["split"] == earlier)
                other = next(row for row in rows if row["split"] == later)
                other["source_id"] = "fixture-eval-second"
                other["near_duplicate_group"] = first["near_duplicate_group"]
                plan = self.changed_index(rows)
                second = copy.deepcopy(plan["sources"][1])
                second["id"] = "fixture-eval-second"
                plan["sources"].append(second)
                result = MODULE.audit(plan, self.folder)
                self.assertFalse(result["prerequisites_ready"], result)
                self.assertTrue(any(issue["section"] == "split_isolation" and "泄漏" in issue["reason"]
                                    for issue in result["issues"]), result["issues"])

    def test_distinct_sources_can_share_local_unit_ids_without_sharing_near_duplicate_groups(self):
        # 来源局部段落编号可以相同，不能把编号巧合等同于内容近重复。
        rows = copy.deepcopy(self.rows)
        other = next(row for row in rows if row["split"] == "heldout")
        other["source_id"] = "fixture-eval-second"
        other["source_unit_id"] = rows[0]["source_unit_id"]
        plan = self.changed_index(rows)
        second = copy.deepcopy(plan["sources"][1])
        second["id"] = "fixture-eval-second"
        plan["sources"].append(second)
        result = MODULE.audit(plan, self.folder)
        self.assertTrue(result["prerequisites_ready"], result["issues"])
        self.assertFalse(result["language_accuracy_valid"])

    def test_changed_ratio_small_heldout_dictionary_gold_and_missing_permissions_block(self):
        changes = []
        for section, field, value in [("gold", "dictionary_derived", True), ("seal", "gold_opened", True),
                                      ("seal", "custodian_id", "fixture-evaluator"), ("freeze", "source_sha256", "0" * 64)]:
            plan = copy.deepcopy(self.plan)
            plan[section][field] = value
            changes.append(plan)
        plan = copy.deepcopy(self.plan)
        plan["family_ratios"] = [80, 10, 10]
        changes.append(plan)
        plan = copy.deepcopy(self.plan)
        plan["sources"][1]["permissions"]["evaluation"] = False
        changes.append(plan)
        changes.append(self.changed_index(self.rows[:100]))
        for plan in changes:
            self.assertFalse(MODULE.audit(plan, self.folder)["prerequisites_ready"])

    def test_artifact_and_seal_or_freeze_tampering_is_detected(self):
        for section, field in [("seal", "family_index_sha256"), ("freeze", "protocol_sha256")]:
            plan = copy.deepcopy(self.plan)
            plan[section][field] = "0" * 64
            self.assertFalse(MODULE.audit(plan, self.folder)["prerequisites_ready"])
        plan = copy.deepcopy(self.plan)
        plan["freeze"]["parameters"]["sha256"] = "0" * 64
        self.assertFalse(MODULE.audit(plan, self.folder)["prerequisites_ready"])
        plan = copy.deepcopy(self.plan)
        plan["freeze"]["frozen_at"] = "2019-01-01T00:00:00Z"
        self.assertFalse(MODULE.audit(plan, self.folder)["prerequisites_ready"])

    def test_prediction_denominator_retains_budget_rejected_and_unsupported(self):
        frozen = MODULE.freeze(self.plan, self.folder, {source["id"]: source for source in self.plan["sources"]})
        value = {"schema_version": 1, "run_started_at": "2020-04-01T00:00:00Z", "bindings": {key: frozen[key] for key in
                 ("parameters_sha256", "source_sha256", "binary_sha256", "lexicon_sha256")},
                 "cases": [{"case_id": row["case_id"], "status": ["ok", "budget", "rejected", "unsupported"][index % 4],
                            "candidates": [], "fallback_preserved": True} for index, row in enumerate(self.rows) if row["split"] == "heldout"]}
        path = self.folder / "predictions.json"
        path.write_text(json.dumps(value), encoding="utf-8")
        result = MODULE.predictions(path, self.rows, frozen)
        self.assertEqual(result["expected_cases"], 2001)
        self.assertEqual(sum(result["statuses"].values()), 2001)
        self.assertFalse(result["language_metrics_computed"])
        for changed in [{**value, "cases": value["cases"][:-1]}, {**value, "cases": value["cases"] + value["cases"][:1]},
                        {**value, "bindings": {}}, {**value, "run_started_at": "2019-01-01T00:00:00Z"}]:
            path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaises(ValueError):
                MODULE.predictions(path, self.rows, frozen)

    def test_non_ok_prediction_cannot_claim_lost_raw_fallback_is_ready(self):
        frozen = MODULE.freeze(self.plan, self.folder, {source['id']: source for source in self.plan['sources']})
        cases = [{'case_id': row['case_id'], 'status': 'ok', 'candidates': [], 'fallback_preserved': True}
                 for row in self.rows if row['split'] == 'heldout']
        path = self.folder / 'lost-fallback.json'
        for status in ['budget', 'rejected', 'unsupported']:
            with self.subTest(status=status):
                rows = copy.deepcopy(cases)
                rows[0].update(status=status, fallback_preserved=False)
                value = {'schema_version': 1, 'run_started_at': '2020-04-01T00:00:00Z',
                         'bindings': {key: frozen[key] for key in
                                      ('parameters_sha256', 'source_sha256', 'binary_sha256', 'lexicon_sha256')},
                         'cases': rows}
                path.write_text(json.dumps(value), encoding='utf-8')
                with self.assertRaisesRegex(ValueError, '原文回退'):
                    MODULE.predictions(path, self.rows, frozen)


if __name__ == "__main__":
    unittest.main()

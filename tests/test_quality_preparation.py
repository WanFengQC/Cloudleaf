"""lineage准备契约攻击测试；全部为自编声明夹具，不作为人工gold。"""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('quality_preparation', ROOT / 'scripts/check_quality_preparation.py')
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.folder = Path(self.temporary.name)
        path = self.folder / 'fixture.txt'
        path.write_text('自编元信息夹具，不是真实批准\n', encoding='utf-8')
        artifact = {'path': path.name, 'sha256': MODULE.base.digest(path)}
        self.plan = {'normalization': 'ascii-lower-umlaut-v1', 'family_index': artifact}
        self.approved = {name: {'role': 'evaluation-corpus', 'artifact': {'sha256': 'a' * 64}}
                         for name in ['source-one', 'source-two']}
        self.rows = [{'source_id': 'source-one', 'source_unit_id': 'canonical-unit', 'split': 'train'},
                     {'source_id': 'source-two', 'source_unit_id': 'other-unit', 'split': 'heldout'}]
        self.value = {'schema_version': 1, 'experiment': 'cloudleaf-quality-preparation-v1',
                      'normalization': self.plan['normalization'], 'family_index_sha256': artifact['sha256'],
                      **{key: artifact for key in ['preprocessing', 'lineage_audit', 'annotation_protocol', 'annotation_review']},
                      'sources': [{'id': name, 'artifact_sha256': 'a' * 64, 'lineage_id': 'shared-lineage',
                                   'preprocessing_sha256': artifact['sha256']} for name in self.approved]}

    def audit(self, value=None, rows=None):
        return MODULE.validate_preparation(value or self.value, self.plan,
                                           self.rows if rows is None else rows, self.approved, self.folder)

    def test_valid_preparation_is_consistency_only_and_covers_every_source(self):
        result = self.audit()
        self.assertFalse(result['semantic_lineage_independently_proven'])
        self.assertEqual(result['evaluation_sources'], 2)
        for key in ['lineage_audit', 'annotation_protocol', 'annotation_review', 'preprocessing']:
            value = copy.deepcopy(self.value)
            del value[key]
            with self.assertRaises((ValueError, KeyError)):
                self.audit(value)

    def test_same_artifact_cannot_change_lineage_by_renaming_source(self):
        value = copy.deepcopy(self.value)
        value['sources'][1]['lineage_id'] = 'renamed'
        with self.assertRaisesRegex(ValueError, '原件.*lineage'):
            self.audit(value)

    def test_same_lineage_unit_cannot_cross_split_through_another_source(self):
        for left, right in [('train', 'dev'), ('train', 'heldout'), ('dev', 'heldout')]:
            with self.subTest(pair=(left, right)):
                rows = copy.deepcopy(self.rows)
                rows[0]['split'], rows[1]['split'] = left, right
                rows[1]['source_unit_id'] = rows[0]['source_unit_id']
                with self.assertRaisesRegex(ValueError, 'lineage.*跨split'):
                    self.audit(rows=rows)

    def test_missing_source_preprocessing_drift_or_artifact_change_is_rejected(self):
        for key in ['missing', 'preprocessing_sha256', 'artifact_sha256']:
            value = copy.deepcopy(self.value)
            if key == 'missing':
                value['sources'].pop()
            else:
                value['sources'][0][key] = 'b' * 64
            with self.assertRaises(ValueError):
                self.audit(value)

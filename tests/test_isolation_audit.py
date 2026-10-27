"""隔离审计自编夹具；不作正式词典、语言规范或人工gold。"""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import test_cedict_source as fixtures
from tools.data_pipeline.audit_isolations import audit_isolations, classify_token


class IsolationAuditTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.CedictSourceTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)

    def prepare(self, entries):
        self.fixture.prepare(entries)
        self.lock_path, self.source = self.fixture.lock_path, self.fixture.source

    def create(self):
        return self.fixture.create()

    def test_classifications_are_shapes_and_source_expressions_not_language_proof(self):
        expected = {'r5': 'erhua_suffix', 'xx5': 'unspecified_reading', 'm2': 'syllabic_nasal',
                    'hm5': 'syllabic_nasal', 'biang2': 'tone_number_not_in_inventory',
                    '11': 'literal_number', 'ABC': 'untoned_ascii_literal', ',': 'separator_comma',
                    '·': 'separator_middle_dot', 'nu:3': 'supported', 'ê4': 'other_source_token'}
        for token, category in expected.items():
            with self.subTest(token=token):
                row = classify_token(token)
                self.assertEqual(row['category'], category)
                self.assertEqual(row['language_validity'], 'not_independently_adjudicated')

    def test_source_rows_release_and_rejections_are_independently_partitioned(self):
        self.prepare(['你好 你好 [ni3 hao3] /hello/', '花兒 花儿 [hua1 r5] /flower/',
                      '標記 标记 [xx5] /mark/', '11區 11区 [11 Qu1] /fixture/',
                      '甲乙 甲乙 [jia3 , yi3] /fixture/',
                      '長詞 长词 [' + ' '.join(['chang2'] * 17) + '] /long/'])
        release = self.create()
        result = audit_isolations(self.lock_path, self.source, release)
        self.assertEqual(result['summary']['source_entries'], 6)
        self.assertEqual(result['summary']['accepted_records'], 1)
        self.assertEqual(result['summary']['isolated_records'], 5)
        self.assertEqual(result['summary']['reason_counts'], {'unsupported_reading':2,
                         'unsupported_cedict_reading':2, 'cedict_runtime_budget':1})
        for row in result['records']:
            self.assertEqual(row['source_line_sha256'], hashlib.sha256(row['raw'].encode('utf-8')).hexdigest())
            self.assertEqual(row['action'], 'retain_isolated_no_runtime_change')
            self.assertEqual(row['language_validity'], 'not_independently_adjudicated')
        self.assertTrue(any(row['classification'] == 'runtime_budget' for row in result['records']))
        self.assertFalse(result['summary']['language_accuracy_valid'])

    def test_recomputed_package_hash_cannot_hide_changed_or_missing_source_rejection(self):
        from tools.data_pipeline.pipeline import digest, read_json, write_json
        self.prepare(['你好 你好 [ni3 hao3] /hello/', '花兒 花儿 [hua1 r5] /flower/'])
        release = self.create()
        original = (release/'rejected.jsonl').read_bytes()
        manifest = read_json(release/'manifest.json')
        for changed in [original.replace(b'r5', b'r4'), b'']:
            (release/'rejected.jsonl').write_bytes(changed)
            current = copy.deepcopy(manifest)
            current['resources']['rejected.jsonl'] = {'bytes':len(changed), 'sha256':digest(changed)}
            write_json(release/'manifest.json', current)
            with self.assertRaises(ValueError):
                audit_isolations(self.lock_path, self.source, release)

    def test_archive_identity_is_checked_before_audit(self):
        self.prepare(['你好 你好 [ni3 hao3] /hello/'])
        release = self.create()
        self.source.write_bytes(self.source.read_bytes() + b'not-the-locked-file')
        with self.assertRaises(ValueError):
            audit_isolations(self.lock_path, self.source, release)

    def test_bad_syntax_and_senses_are_not_called_multiple_token_gaps(self):
        self.prepare(['bad', '甲 甲 [jia3] /valid//', '好 好 [hao3] /good/'])
        result = audit_isolations(self.lock_path, self.source, self.create())
        self.assertEqual([row['classification'] for row in result['records']], ['source_syntax','source_senses'])
        self.assertTrue(all(not row['classification_dimensions'] for row in result['records']))

    def test_conflicting_source_keys_fail_closed_outside_fixed_zero_conflict_scope(self):
        self.prepare(['好 好 [hao3] /one/', '好 好 [hao3] /two/', '甲 甲 [jia3] /good/'])
        with self.assertRaises(ValueError):
            audit_isolations(self.lock_path, self.source, self.create())


if __name__ == '__main__':
    unittest.main()

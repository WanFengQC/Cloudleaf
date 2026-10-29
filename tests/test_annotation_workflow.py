"""人工流程夹具只测声明和泄漏门禁，不是真实gold或真人授权。"""
import copy
import json
from pathlib import Path
import tempfile
import unittest

from tools.data_pipeline.annotation_workflow import annotation_digest, audit, freeze, pending_packet, review_binding_digest

SHA = 'a' * 64


def packet():
    tasks = []
    for index, split in enumerate(['train', 'dev', 'heldout']):
        task = {'id': str(index), 'status': 'completed', 'source_id': 'fixture',
                'source_unit_id': str(index), 'family_id': str(index), 'near_duplicate_group': str(index),
                'split': split, 'author_id': 'fixture-author', 'input_intent': '自编意图' + str(index),
                'pinyin': 'fixture' + str(index), 'acceptable_outputs': ['自编输出' + str(index)],
                'key_trace': ['text:fixture' + str(index), 'Enter'], 'scenario': 'fixture', 'reviews': [],
                'adjudication': None}
        task['reviews'] = [{'reviewer_id': name, 'annotation_sha256': annotation_digest(task),
                            'protocol_sha256': SHA, 'decision': 'approve'} for name in ['fixture-a', 'fixture-b']]
        tasks.append(task)
    value = {'schema_version': 1, 'format': 'cloudleaf-ime-annotation-v1', 'synthetic': True,
            'dictionary_derived': False, 'protocol_sha256': SHA, 'preprocessing_sha256': SHA,
            'lineage_audit_sha256': SHA, 'custodian_id': 'fixture-custodian',
            'sources': [{'id': 'fixture', 'artifact_sha256': SHA, 'lineage_id': 'fixture-family',
                         'version': 'fixture-1', 'author': 'fixture-author',
                         'authorization': {'status': 'approved', 'sha256': SHA,
                                           'scope': ['evaluation', 'modification', 'redistribution']}}],
            'tasks': tasks}
    from tools.data_pipeline.annotation_workflow import source_digest
    for task in tasks:
        task['source_binding_sha256'] = source_digest(value['sources'][0])
        for review in task['reviews']:
            review['annotation_sha256'] = annotation_digest(task)
            review['review_binding_sha256'] = review_binding_digest(value)
    return value


class AnnotationWorkflowTests(unittest.TestCase):
    def test_actual_pending_tasks_cannot_freeze(self):
        value = pending_packet()
        self.assertFalse(audit(value)['structurally_ready'])
        self.assertEqual(len(value['tasks']), 12)
        self.assertTrue(all(task['acceptable_outputs'] == [] for task in value['tasks']))
        with tempfile.TemporaryDirectory() as folder, self.assertRaises(ValueError):
            freeze(value, Path(folder) / 'freeze')

    def test_review_is_bound_to_annotation_and_two_independent_people(self):
        self.assertTrue(audit(packet())['structurally_ready'])
        for mutate in [lambda t: t.update(acceptable_outputs=['改过答案']),
                       lambda t: t['reviews'][0].update(reviewer_id='fixture-author'),
                       lambda t: t['reviews'][1].update(reviewer_id='fixture-a'),
                       lambda t: t['reviews'].pop()]:
            value = packet()
            mutate(value['tasks'][0])
            self.assertFalse(audit(value)['structurally_ready'])

    def test_disagreement_requires_bound_distinct_adjudicator(self):
        value = packet()
        task = value['tasks'][0]
        task['reviews'][1]['decision'] = 'reject'
        self.assertFalse(audit(value)['structurally_ready'])
        task['adjudication'] = {'adjudicator_id': 'fixture-arbiter', 'decision': 'approve',
                                'annotation_sha256': annotation_digest(task), 'protocol_sha256': SHA,
                                'review_binding_sha256': review_binding_digest(value)}
        self.assertTrue(audit(value)['structurally_ready'])
        task['adjudication']['adjudicator_id'] = 'fixture-a'
        self.assertFalse(audit(value)['structurally_ready'])

    def test_global_family_near_lineage_and_alias_isolation(self):
        for field in ['family_id', 'near_duplicate_group', 'source_unit_id', 'pinyin']:
            value = packet()
            value['tasks'][1][field] = value['tasks'][0][field]
            for task in value['tasks']:
                for review in task['reviews']:
                    review['annotation_sha256'] = annotation_digest(task)
            self.assertFalse(audit(value)['structurally_ready'], field)
        value = packet()
        mirror = copy.deepcopy(value['sources'][0])
        mirror.update(id='mirror', lineage_id='renamed-lineage')
        value['sources'].append(mirror)
        value['tasks'][1]['source_id'] = 'mirror'
        self.assertFalse(audit(value)['structurally_ready'])

    def test_authorization_unknown_and_dictionary_answers_are_blocked(self):
        for mutate in [lambda p: p['sources'][0]['authorization'].update(status='unknown'),
                       lambda p: p.update(dictionary_derived=True),
                       lambda p: p.update(custodian_id='fixture-a')]:
            value = packet()
            mutate(value)
            self.assertFalse(audit(value)['structurally_ready'])

    def test_protocol_replacement_invalidates_existing_reviews(self):
        value = packet()
        value['protocol_sha256'] = 'b' * 64
        self.assertFalse(audit(value)['structurally_ready'])

    def test_preprocessing_or_lineage_replacement_invalidates_existing_reviews(self):
        for field in ['preprocessing_sha256', 'lineage_audit_sha256']:
            with self.subTest(field=field):
                value = packet()
                value[field] = 'b' * 64
                self.assertFalse(audit(value)['structurally_ready'])

    def test_legacy_reviews_and_unbound_adjudication_require_explicit_rereview(self):
        value = packet()
        value['tasks'][0]['reviews'][0].pop('review_binding_sha256')
        self.assertFalse(audit(value)['structurally_ready'])
        value = packet()
        task = value['tasks'][0]
        task['reviews'][1]['decision'] = 'reject'
        task['adjudication'] = {'adjudicator_id': 'fixture-arbiter', 'decision': 'approve',
                                'annotation_sha256': annotation_digest(task), 'protocol_sha256': SHA}
        self.assertFalse(audit(value)['structurally_ready'])
        task['adjudication']['review_binding_sha256'] = review_binding_digest(value)
        self.assertTrue(audit(value)['structurally_ready'])

    def test_source_or_grant_replacement_invalidates_existing_reviews(self):
        for field in ['artifact_sha256', 'version', 'authorization']:
            value = packet()
            value['sources'][0][field] = {'status': 'approved', 'sha256': 'b' * 64,
                                        'scope': ['evaluation', 'modification', 'redistribution']} if field == 'authorization' else 'b' * 64
            self.assertFalse(audit(value)['structurally_ready'], field)

    def test_freeze_preserves_split_answer_boundary_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / 'freeze'
            result = freeze(packet(), target)
            public = (target / 'family-index.jsonl').read_text(encoding='utf-8')
            self.assertNotIn('自编输出', public)
            rows = [json.loads(line) for line in public.splitlines()]
            self.assertEqual({row['split'] for row in rows}, {'train', 'dev', 'heldout'})
            self.assertEqual(set(rows[0]), {'case_id', 'family_id', 'source_id', 'source_unit_id',
                                          'near_duplicate_group', 'split', 'normalized_input_sha256', 'task_sha256'})
            self.assertFalse(result['formal_language_ready'])
            self.assertFalse(result['heldout_encrypted'])
            self.assertEqual(result['review_binding_schema_version'], 1)
            self.assertEqual(result['review_binding_sha256'], review_binding_digest(packet()))
            for path in target.iterdir():
                if path.name != 'heldout-tasks.jsonl':
                    self.assertNotIn('自编输出2', path.read_text(encoding='utf-8'), path.name)
            with self.assertRaises(ValueError):
                freeze(packet(), target)

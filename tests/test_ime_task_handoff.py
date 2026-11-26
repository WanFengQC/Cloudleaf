"""方向私有交付协议的机械fixture；无真人许可、标注或正式gold。"""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('ime_handoff', ROOT / 'scripts/check_ime_task_handoff.py')
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def canonical(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')) + '\n').encode('utf-8')


class TaskHandoffTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.folder = Path(self.temporary.name)
        self.metadata = {'schema_version': 1, 'format': 'cloudleaf-ime-annotation-v1', 'synthetic': False,
                         'dictionary_derived': False, 'custodian_id': 'fixture-custodian',
                         'protocol_sha256': '1' * 64, 'preprocessing_sha256': '2' * 64,
                         'lineage_audit_sha256': '3' * 64,
                         'sources': [{'id': 'fixture-source', 'version': 'fixture', 'lineage_id': 'fixture-lineage',
                                      'author': 'fixture-source-author', 'artifact_sha256': '4' * 64,
                                      'authorization': {'status': 'approved', 'sha256': '5' * 64,
                                                        'scope': ['evaluation', 'modification', 'redistribution']}}]}
        self.tasks = []
        self.rows = []
        for index, split in enumerate(['train', 'dev', 'heldout']):
            task = {'id': 'task-' + split, 'status': 'completed', 'source_id': 'fixture-source',
                    'source_unit_id': 'unit-' + split, 'family_id': 'family-' + split,
                    'near_duplicate_group': 'near-' + split, 'split': split,
                    'source_binding_sha256': hashlib.sha256(canonical(self.metadata['sources'][0])).hexdigest(),
                    'author_id': 'fixture-author', 'input_intent': 'fixture-intent-' + split,
                    'pinyin': 'fixture-input-' + split, 'acceptable_outputs': ['fixture-output-' + split],
                    'key_trace': ['fixture-event-' + split], 'scenario': 'fixture-scene', 'adjudication': None}
            bound = MODULE.annotation_digest(task)
            task['reviews'] = [{'reviewer_id': name, 'decision': 'approve', 'annotation_sha256': bound,
                                'protocol_sha256': self.metadata['protocol_sha256'],
                                'review_binding_sha256': MODULE.review_binding_digest(self.metadata)}
                               for name in ['fixture-reviewer-a', 'fixture-reviewer-b']]
            self.tasks.append(task)
            self.rows.append({'case_id': task['id'], 'source_id': task['source_id'],
                              'source_unit_id': task['source_unit_id'], 'family_id': task['family_id'],
                              'near_duplicate_group': task['near_duplicate_group'], 'split': split,
                              'normalized_input_sha256': MODULE.base.normalized_digest(task['pinyin']), 'task_sha256': bound})
        self.metadata['task_ids'] = [row['id'] for row in self.tasks]
        self.plan = {'family_index': {'sha256': '6' * 64}}
        self.approved = {'fixture-source': {'role': 'evaluation-corpus', 'version': 'fixture', 'artifact': {'sha256': '4' * 64}}}
        self.preparation = {'sources': [{'id': 'fixture-source', 'lineage_id': 'fixture-lineage'}],
                            'annotation_protocol': {'sha256': '1' * 64}, 'preprocessing': {'sha256': '2' * 64},
                            'lineage_audit': {'sha256': '3' * 64}}
        self.context = {'schema_version': 1, 'family_index_sha256': '6' * 64,
                        'review_binding_sha256': MODULE.review_binding_digest(self.metadata),
                        'all_case_intent_groups': [{'case_id': row['case_id'], 'intent_family_id': 'intent-' + row['split']}
                                                   for row in self.rows],
                        'cases': [{'case_id': row['case_id'], 'task_sha256': row['task_sha256'],
                                   'intent_family_id': 'intent-' + row['split'], 'context_kind': 'none',
                                   'confirmed_left_context': ''} for row in self.rows if row['split'] != 'heldout']}
        self.manifest = {'schema_version': 1, 'experiment': 'cloudleaf-ime-task-handoff-v1',
                         'family_index_sha256': '6' * 64}
        self.refresh()

    def put(self, name, value):
        path = self.folder / name
        path.write_bytes(value)
        return {'path': name, 'sha256': MODULE.base.digest(path)}

    def refresh(self):
        self.manifest['metadata'] = self.put('metadata.json', canonical(self.metadata))
        self.manifest['context_review'] = self.put('context.json', canonical(self.context))
        self.manifest['visible_tasks'] = {split: self.put(split + '.jsonl', b''.join(canonical(task) for task in self.tasks if task['split'] == split))
                                          for split in ['train', 'dev']}

    def audit(self):
        return MODULE.validate_handoff(self.manifest, self.plan, self.rows, self.approved, self.preparation, self.folder)

    def test_complete_visible_handoff_is_consistency_only_and_never_needs_heldout_plaintext(self):
        result = self.audit()
        self.assertEqual(result['visible_tasks'], 2)
        self.assertFalse(result['heldout_answers_opened'])
        self.assertFalse(result['human_identity_or_semantic_groups_proven'])
        self.assertFalse((self.folder / 'heldout.jsonl').exists())

    def test_tampered_task_or_task_summary_is_rejected(self):
        for field in ['input_intent', 'pinyin', 'acceptable_outputs', 'key_trace', 'source_binding_sha256']:
            with self.subTest(field=field):
                original = copy.deepcopy(self.tasks[0])
                self.tasks[0][field] = ['changed'] if isinstance(original[field], list) else 'changed'
                self.refresh()
                with self.assertRaises(ValueError):
                    self.audit()
                self.tasks[0] = original
        self.rows[0]['task_sha256'] = 'a' * 64
        self.refresh()
        with self.assertRaisesRegex(ValueError, '摘要'):
            self.audit()

    def test_new_preprocessing_or_lineage_requires_new_review_binding(self):
        for field in ['preprocessing_sha256', 'lineage_audit_sha256']:
            with self.subTest(field=field):
                old = self.metadata[field]
                self.metadata[field] = 'e' * 64
                self.context['review_binding_sha256'] = MODULE.review_binding_digest(self.metadata)
                self.refresh()
                with self.assertRaisesRegex(ValueError, '审核'):
                    self.audit()
                self.metadata[field] = old

    def test_global_intent_group_cannot_cross_split_even_if_source_family_different(self):
        self.context['all_case_intent_groups'][-1]['intent_family_id'] = 'intent-train'
        self.refresh()
        with self.assertRaisesRegex(ValueError, '意图.*跨split'):
            self.audit()

    def test_context_annotation_requires_fixed_task_and_explicit_context(self):
        for patch in [{'task_sha256': 'a' * 64}, {'context_kind': 'confirmed-left'},
                      {'context_kind': 'none', 'confirmed_left_context': 'unexpected'}]:
            original = copy.deepcopy(self.context['cases'][0])
            self.context['cases'][0].update(patch)
            self.refresh()
            with self.assertRaises(ValueError):
                self.audit()
            self.context['cases'][0] = original

    def test_missing_visible_task_duplicate_case_or_public_answers_cannot_pass(self):
        self.tasks.pop(0)
        self.refresh()
        with self.assertRaisesRegex(ValueError, '分母'):
            self.audit()
        self.metadata['tasks'] = [{'acceptable_outputs': ['heldout-leak']}]
        self.refresh()
        with self.assertRaisesRegex(ValueError, '公共元信息'):
            self.audit()

    def test_old_review_missing_composite_binding_and_nonindependent_reviewers_rejected(self):
        self.tasks[0]['reviews'][0].pop('review_binding_sha256')
        self.refresh()
        with self.assertRaisesRegex(ValueError, '审核'):
            self.audit()
        self.tasks[0]['reviews'][0]['review_binding_sha256'] = MODULE.review_binding_digest(self.metadata)
        self.tasks[0]['reviews'][0]['reviewer_id'] = self.tasks[0]['author_id']
        self.refresh()
        with self.assertRaisesRegex(ValueError, '独立'):
            self.audit()

    def test_rejected_review_requires_independent_current_binding_adjudication(self):
        self.tasks[0]['reviews'][0]['decision'] = 'reject'
        self.refresh()
        with self.assertRaisesRegex(ValueError, '仲裁'):
            self.audit()
        self.tasks[0]['adjudication'] = {'adjudicator_id': 'fixture-arbiter', 'decision': 'approve',
                                        'annotation_sha256': self.rows[0]['task_sha256'],
                                        'protocol_sha256': self.metadata['protocol_sha256'],
                                        'review_binding_sha256': MODULE.review_binding_digest(self.metadata)}
        self.refresh()
        self.assertEqual(self.audit()['visible_tasks'], 2)
        self.tasks[0]['adjudication']['review_binding_sha256'] = 'b' * 64
        self.refresh()
        with self.assertRaisesRegex(ValueError, '仲裁'):
            self.audit()


if __name__ == '__main__':
    unittest.main()

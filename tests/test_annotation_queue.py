"""作者草稿工具的机械格式测试，不是实际授权、人员或gold。"""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


def module():
    sys.path.insert(0, str(ROOT / 'scripts'))
    spec = importlib.util.spec_from_file_location('annotation_queue', ROOT / 'scripts/manage_ime_annotation_queue.py')
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


class AnnotationQueueTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.folder = Path(self.temporary.name)
        self.packet = {'schema_version': 1, 'format': 'cloudleaf-ime-annotation-v1', 'synthetic': False,
                       'dictionary_derived': False, 'custodian_id': None, 'protocol_sha256': None,
                       'preprocessing_sha256': None, 'lineage_audit_sha256': None, 'sources': [],
                       'tasks': [{'id': 'fixture-task', 'status': 'pending', 'split': None,
                                  **{key: None for key in ['source_id', 'source_unit_id', 'family_id', 'near_duplicate_group',
                                      'source_binding_sha256', 'author_id', 'input_intent', 'pinyin']},
                                  'acceptable_outputs': [], 'key_trace': [], 'scenario': '机械fixture', 'reviews': [], 'adjudication': None}]}

    def put(self, name, value):
        path = self.folder / name
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
        return path

    def exported(self):
        source = self.put('source.json', self.packet)
        return module().export_queue(source, hashlib.sha256(source.read_bytes()).hexdigest())

    def complete(self):
        implementation = module()
        queue = self.exported()
        packet = queue['packet']
        for key, digit in zip(implementation.handoff.BINDINGS, ['1', '2', '3']):
            packet[key] = digit * 64
        path = self.folder / 'fixture-authorization.txt'
        path.write_text('仅机械fixture，不是真实许可或人工签名。', encoding='utf-8')
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        source = {'id': 'fixture-source', 'version': 'fixture', 'author': 'fixture-source-author',
                  'lineage_id': 'fixture-lineage', 'artifact_sha256': '4' * 64,
                  'authorization': {'status': 'approved', 'sha256': digest,
                                    'scope': ['evaluation', 'modification', 'redistribution']}}
        packet['sources'] = [source]
        queue['authorization_files'] = {'fixture-source': {'path': path.name, 'sha256': digest}}
        task = packet['tasks'][0]
        task.update(source_id='fixture-source', source_unit_id='fixture-unit', family_id='fixture-family',
                    near_duplicate_group='fixture-near', author_id='fixture-author', input_intent='不得输出的意图',
                    pinyin='private-fixture-input', acceptable_outputs=['不得输出的答案'], key_trace=['fixture-key'],
                    source_binding_sha256=hashlib.sha256(implementation.handoff.canonical(source)).hexdigest())
        task['reviews'] = [{'reviewer_id': person, 'decision': 'approve',
                            'annotation_sha256': implementation.handoff.annotation_digest(task),
                            'protocol_sha256': packet['protocol_sha256'],
                            'review_binding_sha256': implementation.handoff.review_binding_digest(packet)}
                           for person in ['fixture-reviewer-a', 'fixture-reviewer-b']]
        packet['custodian_id'] = 'fixture-custodian'
        return queue

    def report(self, queue):
        return module().status(self.put('queue.json', queue))

    def test_repository_evaluator_is_pending_not_an_invented_team(self):
        plan = json.loads((ROOT / 'config/evaluation/language-readiness-v1.json').read_text(encoding='utf-8'))
        self.assertIsNone(plan['evaluator_id'])

    def test_export_preserves_pending_tasks_without_filling_people_or_answers(self):
        queue = self.exported()
        self.assertEqual(queue['usage'], 'internal-development-only')
        self.assertEqual(queue['packet'], self.packet)
        report = self.report(queue)
        self.assertEqual(report['counts']['tasks'], 1)
        self.assertEqual(report['counts']['personal_annotations_complete'], 0)
        self.assertFalse(report['formal_ready'])
        self.assertIsNone(report['top1'])

    def test_complete_personal_draft_and_matching_declarations_never_become_formal_gold(self):
        report = self.report(self.complete())
        self.assertEqual(report['counts']['personal_annotations_complete'], 1)
        self.assertEqual(report['counts']['current_review_declarations_consistent'], 1)
        self.assertTrue(report['custodian_declaration_consistent'])
        self.assertFalse(report['human_identity_or_signed_authorization_proven'])
        self.assertFalse(report['formal_ready'])
        self.assertEqual([report[key] for key in ['top1', 'top5', 'mrr']], [None, None, None])
        text = json.dumps(report, ensure_ascii=False)
        for private in ['不得输出的意图', '不得输出的答案', 'private-fixture-input']:
            self.assertNotIn(private, text)

    def test_author_reviewer_duplicate_and_empty_protocols_do_not_count_as_reviewed(self):
        original = self.complete()
        for patch in ['same-reviewer', 'self-review', 'missing-protocol']:
            queue = copy.deepcopy(original)
            task = queue['packet']['tasks'][0]
            if patch == 'same-reviewer':
                task['reviews'][1]['reviewer_id'] = task['reviews'][0]['reviewer_id']
            elif patch == 'self-review':
                task['reviews'][0]['reviewer_id'] = task['author_id']
            else:
                queue['packet']['preprocessing_sha256'] = None
            with self.subTest(patch=patch):
                self.assertEqual(self.report(queue)['counts']['current_review_declarations_consistent'], 0)

    def test_authorization_missing_tampered_scope_and_source_changes_are_reported(self):
        original = self.complete()
        for patch in ['missing-file', 'wrong-sha', 'missing-scope', 'changed-source']:
            queue = copy.deepcopy(original)
            if patch == 'missing-file':
                queue['authorization_files']['fixture-source']['path'] = 'not-present'
            elif patch == 'wrong-sha':
                queue['authorization_files']['fixture-source']['sha256'] = 'a' * 64
            elif patch == 'missing-scope':
                queue['packet']['sources'][0]['authorization']['scope'] = ['evaluation']
            else:
                queue['packet']['sources'][0]['version'] = 'changed'
            with self.subTest(patch=patch):
                report = self.report(queue)
                if patch == 'changed-source':
                    self.assertEqual(report['counts']['current_review_declarations_consistent'], 0)
                else:
                    self.assertFalse(report['authorizations']['fixture-source']['file_and_scope_consistent'])
                self.assertFalse(report['formal_ready'])

    def test_changed_annotation_and_rejection_require_current_independent_arbitration(self):
        queue = self.complete()
        task = queue['packet']['tasks'][0]
        task['input_intent'] = 'changed'
        self.assertEqual(self.report(queue)['counts']['current_review_declarations_consistent'], 0)
        queue = self.complete()
        task = queue['packet']['tasks'][0]
        task['reviews'][0]['decision'] = 'reject'
        self.assertEqual(self.report(queue)['counts']['current_review_declarations_consistent'], 0)
        implementation = module()
        task['adjudication'] = {'adjudicator_id': 'fixture-arbiter', 'decision': 'approve',
                                'annotation_sha256': implementation.handoff.annotation_digest(task),
                                'protocol_sha256': queue['packet']['protocol_sha256'],
                                'review_binding_sha256': implementation.handoff.review_binding_digest(queue['packet'])}
        self.assertEqual(self.report(queue)['counts']['current_review_declarations_consistent'], 1)
        task['adjudication']['adjudicator_id'] = task['author_id']
        self.assertEqual(self.report(queue)['counts']['current_review_declarations_consistent'], 0)

    def test_assigned_packet_missing_original_slot_and_nonindependent_custodian_are_rejected_or_pending(self):
        implementation = module()
        for split in ['train', 'dev', 'heldout']:
            self.packet['tasks'][0]['split'] = split
            path = self.put('source.json', self.packet)
            with self.subTest(split=split), self.assertRaisesRegex(ValueError, '未划分'):
                implementation.export_queue(path, hashlib.sha256(path.read_bytes()).hexdigest())
        self.packet['tasks'][0]['split'] = None
        queue = self.complete()
        queue['packet']['custodian_id'] = 'fixture-author'
        self.assertFalse(self.report(queue)['custodian_declaration_consistent'])
        queue['packet']['tasks'] = []
        with self.assertRaisesRegex(ValueError, '原始任务'):
            self.report(queue)

    def test_cli_hash_and_existing_output_are_rejected_without_replacing_bytes(self):
        source = self.put('source.json', self.packet)
        output = self.folder / 'export.json'
        output.write_bytes(b'keep-existing')
        command = [sys.executable, str(ROOT / 'scripts/manage_ime_annotation_queue.py'), 'export',
                   '--packet', str(source), '--sha256', hashlib.sha256(source.read_bytes()).hexdigest(), '--output', str(output)]
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(output.read_bytes(), b'keep-existing')
        with self.assertRaisesRegex(ValueError, 'SHA'):
            module().export_queue(source, 'a' * 64)


if __name__ == '__main__':
    unittest.main()

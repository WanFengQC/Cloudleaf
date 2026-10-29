"""临时材料只验证准入实现，不是实际回滚、授权或长期验收。"""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import check_windows_soak_admission as ADMISSION


def save_material(folder, name, value):
    path = folder / name
    path.write_text(json.dumps(value, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
    return {'path': name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def fixture_identity():
    return {'configuration': {'profile': '8h'}, 'environment': {'os': 'Windows-fixture', 'architecture': 'AMD64'}}


def prepared_request(folder, identity=None):
    request = json.loads((ROOT / 'docs/acceptance/cl007-round8-environment.json').read_text(encoding='utf-8'))
    request.update(status='ready_for_preflight', user_reported_environment_status='prepared')
    request['environment'].update(vm_id='fixture-not-real-vm', windows_version_and_build='Windows-fixture', architecture='AMD64',
                                  snapshot_id='fixture-snapshot', snapshot_storage_sha256='a' * 64)
    request['people'].update(environment_owner='fixture-single-owner', test_operator='fixture-single-owner')
    restore = {'schema_version': 1, 'kind': 'cloudleaf-windows-restore-exercise', 'status': 'passed',
               'environment_id': 'fixture-not-real-vm', 'snapshot_id': 'fixture-snapshot', 'snapshot_storage_sha256': 'a' * 64,
               'operator': 'fixture-single-owner', 'performed_at_utc': '2026-01-01T00:00:00+00:00'}
    authorization = {'schema_version': 2, 'kind': 'cloudleaf-windows-soak-execution-authorization', 'status': 'approved',
                     'environment_id': 'fixture-not-real-vm', 'snapshot_id': 'fixture-snapshot', 'operator': 'fixture-single-owner',
                     'authorized_by': 'fixture-single-owner', 'authorized_at_utc': '2026-01-01T01:00:00+00:00',
                     'allowed_profiles': ['8h'],
                     'execution_identity_sha256': ADMISSION.execution_identity_sha256(identity if identity is not None else fixture_identity())}
    request['environment']['restore_exercise_evidence'] = save_material(folder, 'fixture-restore.json', restore)
    request['signatures_and_authorization_evidence'] = save_material(folder, 'fixture-authorization.json', authorization)
    return request


class AdmissionTests(unittest.TestCase):
    def test_actual_pending_environment_remains_blocked_and_never_starts_process(self):
        path = ROOT / 'docs/acceptance/cl007-round8-environment.json'
        result = ADMISSION.check_request(json.loads(path.read_text(encoding='utf-8')), path.parent)
        self.assertFalse(result['ready'])
        self.assertFalse(result['child_started'])
        self.assertFalse(result['is_execution_authorization'])
        self.assertGreater(len(result['issues']), 5)

    def test_single_owner_can_supply_consistent_materials_without_becoming_independent_review(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            result = ADMISSION.check_request(prepared_request(folder), folder, fixture_identity())
            self.assertTrue(result['ready'], result['issues'])
            self.assertEqual(len(result['checked_files']), 2)
            self.assertFalse(result['declarations_independently_verified'])
            self.assertFalse(result['is_execution_authorization'])
            self.assertFalse(result['long_term_acceptance'])

    def test_each_required_field_or_evidence_is_required(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            request = prepared_request(folder)
            for group, field in [('environment', name) for name in ['vm_id', 'windows_version_and_build', 'architecture', 'snapshot_id',
                                                                    'snapshot_storage_sha256', 'restore_exercise_evidence']] + [
                                   ('people', 'environment_owner'), ('people', 'test_operator'),
                                   (None, 'signatures_and_authorization_evidence')]:
                with self.subTest(group=group, field=field):
                    changed = copy.deepcopy(request)
                    (changed[group] if group else changed)[field] = None
                    self.assertFalse(ADMISSION.check_request(changed, folder, fixture_identity())['ready'])

    def test_missing_modified_or_wrongly_bound_material_does_not_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            request = prepared_request(folder)
            path = folder / request['environment']['restore_exercise_evidence']['path']
            original = path.read_bytes()
            path.write_bytes(original + b' ')
            self.assertFalse(ADMISSION.check_request(request, folder, fixture_identity())['ready'])
            path.unlink()
            self.assertFalse(ADMISSION.check_request(request, folder, fixture_identity())['ready'])
            restored = json.loads(original)
            for field, value in [('environment_id', 'wrong-vm'), ('snapshot_id', 'old-snapshot'), ('status', 'planned'),
                                 ('operator', 'different-person'), ('performed_at_utc', '2026-01-01')]:
                with self.subTest(field=field):
                    changed = dict(restored, **{field: value})
                    request['environment']['restore_exercise_evidence'] = save_material(folder, path.name, changed)
                    self.assertFalse(ADMISSION.check_request(request, folder, fixture_identity())['ready'])

    def test_bound_request_checks_full_file_hash_os_architecture_and_profile(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            identity = {'configuration': {'profile': '8h'}, 'environment': {'os': 'Windows-fixture', 'architecture': 'AMD64'}}
            request = prepared_request(folder, identity)
            reference = save_material(folder, 'request.json', request)
            result = ADMISSION.check_bound_request(reference, folder, identity)
            self.assertTrue(result['ready'], result['issues'])
            self.assertEqual(len(result['checked_files']), 3)
            for configuration, environment in [({'profile': '24h'}, identity['environment']),
                                                (identity['configuration'], {'os': 'Windows-other', 'architecture': 'AMD64'}),
                                                (identity['configuration'], {'os': 'Windows-fixture', 'architecture': 'ARM64'})]:
                self.assertFalse(ADMISSION.check_bound_request(reference, folder, {'configuration': configuration, 'environment': environment})['ready'])
            reference['sha256'] = 'b' * 64
            self.assertFalse(ADMISSION.check_bound_request(reference, folder, identity)['ready'])

    def test_standalone_consistent_materials_do_not_claim_execution_readiness(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            result = ADMISSION.check_request(prepared_request(folder), folder)
            self.assertTrue(result['materials_consistent'])
            self.assertFalse(result['ready'])
            self.assertFalse(result['execution_identity_binding_checked'])
            self.assertFalse(result['child_started'])

    def test_old_authorization_or_missing_identity_digest_cannot_be_upgraded(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            request = prepared_request(folder)
            reference = request['signatures_and_authorization_evidence']
            original = json.loads((folder / reference['path']).read_bytes())
            for field, value in [('schema_version', 1), ('execution_identity_sha256', None), ('execution_identity_sha256', 'b' * 64)]:
                with self.subTest(field=field, value=value):
                    changed = dict(original, **{field: value})
                    request['signatures_and_authorization_evidence'] = save_material(folder, reference['path'], changed)
                    self.assertFalse(ADMISSION.check_request(request, folder, fixture_identity())['ready'])

    def test_identity_digest_is_canonical_and_only_excludes_post_approval_digest(self):
        identity = fixture_identity()
        digest = ADMISSION.execution_identity_sha256(identity)
        reordered = {'environment': identity['environment'], 'configuration': identity['configuration']}
        self.assertEqual(ADMISSION.execution_identity_sha256(reordered), digest)
        self.assertEqual(ADMISSION.execution_identity_sha256(dict(identity, execution_approval_sha256='a' * 64)), digest)
        self.assertNotEqual(ADMISSION.execution_identity_sha256(dict(identity, binary_sha256='a' * 64)), digest)

    def test_cli_pending_report_is_read_only_and_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'admission.json'
            command = [sys.executable, str(ROOT / 'scripts/check_windows_soak_admission.py'), '--request',
                       str(ROOT / 'docs/acceptance/cl007-round8-environment.json'), '--output', str(output)]
            result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(result.returncode, 2)
            original = output.read_bytes()
            self.assertFalse(json.loads(original)['child_started'])
            repeated = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(repeated.returncode, 2)
            self.assertEqual(output.read_bytes(), original)


if __name__ == '__main__':
    unittest.main()

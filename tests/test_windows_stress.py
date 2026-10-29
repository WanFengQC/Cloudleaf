"""长期诊断的配置、原文落盘及身份隔离；真实COM另由实际Windows子进程验证。"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
SPEC = importlib.util.spec_from_file_location('windows_stress', SCRIPTS / 'stress_windows_faults.py')
STRESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STRESS)


def trace_rows():
    values = [{'experiment': 'r6-owned-key', 'cycle': 1, 'instance': 35, 'epoch': 2,
               'sequence': number, 'vk': key, 'lparam': 1, 'admission_ns': number,
               'commit_complete_ns': number + 10}
              for number, key in enumerate([78, 73, 72, 65, 79, 32], 1)]
    values.append({'experiment': 'r6-cycle', 'cycle': 1, 'healthy_writes': 6,
                   'fifo_and_budget_verified': True})
    values.append({'experiment': 'r5-pinned-availability', 'healthy_cycles': 1, 'healthy_writes': 6,
                   'faulted_owners': 32, 'unknown_replays': 0, 'remaining_hosts': 32,
                   'remaining_batches': 32, 'pinned_budget_unchanged': True, 'steady_elapsed_ns': 1_000_000_000})
    return values


def summarize_rows(folder, rows):
    path = folder / 'events.jsonl'
    path.write_text(''.join(json.dumps(row) + '\n' for row in rows), encoding='utf-8')
    return STRESS.latency_summary(path)


class StressEvidenceTests(unittest.TestCase):
    def test_long_profiles_are_exact_contiguous_durations_and_bounded_still_compatible(self):
        self.assertEqual(STRESS.configuration('8h', None, None, 60, 1)['seconds'], 28800)
        self.assertEqual(STRESS.configuration('24h', None, None, 60, 1)['cycle_ms'], 1000)
        self.assertEqual(STRESS.configuration('bounded', 180, None, 5, .2)['cycle_ms'], 0)
        with self.assertRaises(ValueError):
            STRESS.configuration('8h', 3, None, 60, 1)
        with self.assertRaises(ValueError):
            STRESS.configuration('bounded', 179, None, 5, .2)

    def test_diagnostic_is_explicit_and_cannot_claim_long_acceptance(self):
        value = STRESS.configuration('diagnostic', 2, 10, .1, .05)
        self.assertEqual(value['seconds'], 2)
        self.assertFalse(value['long_profile'])
        with self.assertRaises(ValueError):
            STRESS.configuration('diagnostic', 28800, None, 1, 1)

    def test_checkpoint_preserves_jsonl_prefix_and_rejects_changed_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            journal = STRESS.Journal(folder)
            journal.append('events', {'sequence': 1, 'text': '自有测试'})
            journal.append('samples', {'handles': 7})
            journal.checkpoint({'state': 'running', 'completed_cycles': 1})
            self.assertEqual(json.loads((folder / 'checkpoint.json').read_text(encoding='utf-8'))['completed_cycles'], 1)
            self.assertEqual(json.loads((folder / 'events.jsonl').read_text(encoding='utf-8'))['sequence'], 1)
            journal.checkpoint({'state': 'interrupted', 'completed_cycles': 1})
            journal.close()
            self.assertEqual(len((folder / 'checkpoints.jsonl').read_text(encoding='utf-8').splitlines()), 2)
            STRESS.verify_identity({'binary': 'a'}, {'binary': 'a'})
            with self.assertRaises(ValueError):
                STRESS.verify_identity({'binary': 'a'}, {'binary': 'b'})

    def test_raw_latency_distributions_keep_event_denominator_and_unknown_counts(self):
        with tempfile.TemporaryDirectory() as temporary:
            journal = STRESS.Journal(Path(temporary))
            for sequence, latency in enumerate([4, 12, 20], 1):
                journal.append('events', {'experiment': 'r6-owned-key', 'sequence': sequence,
                                          'admission_ns': latency, 'commit_complete_ns': latency + 5})
            journal.close()
            result = STRESS.latency_summary(Path(temporary) / 'events.jsonl')
            self.assertEqual(result['events'], 3)
            self.assertEqual(result['admission_ns']['p50'], 12)
            self.assertIsNone(result['external_com_reference_count'])

    def test_valid_json_missing_key_or_cycle_cannot_be_complete_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            for rows in [trace_rows()[1:], trace_rows()[:6] + trace_rows()[7:]]:
                with self.subTest(rows=rows):
                    result = summarize_rows(folder, rows)
                    self.assertTrue(result['raw_errors'])

    def test_duplicate_sequence_or_changed_context_is_retained_as_trace_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            for field, value in [('sequence', 1), ('instance', 36), ('cycle', 2)]:
                with self.subTest(field=field):
                    rows = trace_rows()
                    rows[1][field] = value
                    self.assertTrue(summarize_rows(folder, rows)['raw_errors'])

    def test_complete_trace_keeps_denominator_but_missing_final_is_partial(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            result = summarize_rows(folder, trace_rows())
            self.assertFalse(result['raw_errors'])
            self.assertTrue(result['trace_complete'])
            self.assertEqual(result['cycles'], 1)
            self.assertEqual(result['events'], 6)
            partial = summarize_rows(folder, trace_rows()[:-1])
            self.assertFalse(partial['trace_complete'])
            self.assertEqual(partial['events'], 6)

    def test_long_run_admission_requires_explicit_matching_isolated_approval(self):
        identity = {'configuration': STRESS.configuration('8h', None, None, 60, 1),
                    'implementation_commit': 'fixture', 'environment': {'machine_id_sha256': 'fixture'}}
        missing = STRESS.execution_preflight(identity, None)
        self.assertFalse(missing['ready'])
        approval = {'status': 'approved', 'isolated_windows_environment': True, 'environment_id': 'test-vm',
                    'operator': 'fixture-operator', 'snapshot': 'fixture-snapshot', 'rollback_plan': 'fixture-rollback',
                    'authorization_reference': 'fixture-only-not-real-approval', 'identity': identity}
        approved = STRESS.execution_preflight(identity, approval)
        self.assertTrue(approved['ready'])
        self.assertFalse(approved['declarations_independently_verified'])
        changed = dict(identity, implementation_commit='changed')
        self.assertFalse(STRESS.execution_preflight(changed, approval)['ready'])
        approval['isolated_windows_environment'] = False
        self.assertFalse(STRESS.execution_preflight(identity, approval)['ready'])

    @unittest.skipUnless(os.name == 'nt', '需要真实Windows进程句柄')
    def test_process_identity_refuses_live_child_and_recognizes_its_actual_exit(self):
        process = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'])
        try:
            identity = STRESS.process_identity(process)
            self.assertEqual(STRESS.process_status(identity), 'alive')
            with self.assertRaises(ValueError):
                STRESS.require_dead(identity)
        finally:
            process.terminate()
            process.wait(timeout=10)
        self.assertIn(STRESS.process_status(identity), {'dead', 'reused'})
        STRESS.require_dead(identity)


if __name__ == '__main__':
    unittest.main()

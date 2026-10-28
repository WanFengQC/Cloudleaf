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

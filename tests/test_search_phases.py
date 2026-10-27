"""阶段与失败分母的独立分析语义测试；正文均为机械fixture。"""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('search_phases', ROOT / 'scripts/evaluate_search_phases.py')
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def sample(rejected=False):
    phases = {name: None if name == 'context_rerank' or (rejected and name not in ('query_total', 'normalize'))
              else {'ns': 10, 'calls': 1} for name in MODULE.PHASES}
    return {'plain_ns': 50, 'profile_outer_ns': 100, 'plain_first': True, 'equal': True,
            'profile': {'clock_reads': sum(value['calls'] * 2 for value in phases.values() if value), 'phases': phases}}


class PhaseTests(unittest.TestCase):
    def test_historical_protocol_or_any_full_configuration_drift_is_rejected(self):
        old = {'evidence': {'runtime': {'sha256': 'a' * 64}}, 'cases_sha256': 'b' * 64,
               'protocol_sha256': 'c' * 64, 'runs': {}}
        for label, (method, width, edges, expansions, resident) in MODULE.CONFIGURATIONS.items():
            old['runs'][label] = {'configuration': {'method': method, 'beam_width': width, 'max_edges': edges,
                    'max_expansions': expansions, 'max_resident_paths': resident, 'max_input_bytes': 512,
                    'max_letters': 256, 'output_k': 5, 'ranker': 'Structural'}}
        MODULE.validate_historical_identity(old, 'a' * 64, 'b' * 64, 'c' * 64)
        changed = copy.deepcopy(old)
        changed['protocol_sha256'] = 'd' * 64
        with self.assertRaisesRegex(ValueError, '协议'):
            MODULE.validate_historical_identity(changed, 'a' * 64, 'b' * 64, 'c' * 64)
        for field in old['runs']['reference']['configuration']:
            changed = copy.deepcopy(old)
            changed['runs']['reference']['configuration'][field] = 'drift'
            with self.assertRaisesRegex(ValueError, '配置'):
                MODULE.validate_historical_identity(changed, 'a' * 64, 'b' * 64, 'c' * 64)

    def test_failed_child_stdout_prefix_and_stderr_are_retained_before_raising(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            stdout, stderr = folder / 'stdout.json', folder / 'stderr.txt'
            command = [sys.executable, '-c', 'import sys;print(\'{"partial":true,"cases":[{"id":"first"}]}\');print("fixture failure",file=sys.stderr);sys.exit(3)']
            with self.assertRaisesRegex(RuntimeError, 'fixture failure'):
                MODULE.measure(command, stdout_path=stdout, stderr_path=stderr)
            self.assertTrue(json.loads(stdout.read_text(encoding='utf-8'))['partial'])
            self.assertIn('fixture failure', stderr.read_text(encoding='utf-8'))

    def test_nested_stages_are_not_added_or_subtracted_and_missing_is_not_zero(self):
        value = sample()
        MODULE.validate_sample(value, False, 0)
        self.assertIsNone(value['profile']['phases']['context_rerank'])
        value['profile']['phases']['query_total']['ns'] = 1
        # 独立时钟测量不靠相加/相减推导未观测CPU成本，分析器保留事实。
        MODULE.validate_sample(value, False, 0)

    def test_rejected_input_retains_normalize_and_total_but_not_fake_later_stages(self):
        value = sample(True)
        MODULE.validate_sample(value, True, 0)
        value['profile']['phases']['lattice'] = {'ns': 0, 'calls': 1}
        value['profile']['clock_reads'] += 2
        with self.assertRaisesRegex(ValueError, '未进入'):
            MODULE.validate_sample(value, True, 0)

    def test_missing_sample_read_clock_drift_or_behavior_change_is_rejected(self):
        for change in ['equal', 'clock_reads', 'phase']:
            value = sample()
            if change == 'equal':
                value['equal'] = False
            elif change == 'clock_reads':
                value['profile']['clock_reads'] += 1
            else:
                del value['profile']['phases']['traversal']
            with self.assertRaises(ValueError):
                MODULE.validate_sample(value, False, 0)

    def test_budget_and_rejected_remain_in_full_denominator(self):
        cases = [{'id': name, 'input': 'a', 'family': name, 'kind': 'fixture'} for name in ['budget', 'rejected']]
        run = {'language_accuracy_valid': False, 'iterations_per_case': 1, 'warmup_per_case': 0,
               'cases': [{**cases[0], 'samples': [sample()], 'result': {'status': 'ok', 'statistics': {'budget': 'Expansions'}}},
                         {**cases[1], 'samples': [sample(True)], 'result': {'status': 'rejected'}}]}
        result = MODULE.validate_run(run, cases, 1, 0)
        self.assertEqual(result['outcomes'], {'budget': 1, 'rejected': 1})
        self.assertFalse(result['stage_sums_valid'])
        self.assertIsNone(result['exclusive_cpu_cost'])
        changed = copy.deepcopy(run)
        changed['cases'].pop()
        with self.assertRaisesRegex(ValueError, '分母'):
            MODULE.validate_run(changed, cases, 1, 0)


class ActualProfileTests(unittest.TestCase):
    def test_actual_release_host_records_each_failure_sample_and_matches_old_serialization(self):
        from evaluate_real_lexicon import build_host as old_host
        binary, original = MODULE.build_host(), old_host()
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            runtime, cases = folder / 'runtime.tsv', folder / 'cases.tsv'
            runtime.write_text('# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\ta\t甲\t\tunknown\t\tfixture\t1\tMIT\nb\ta\t乙\t\tunknown\t\tfixture\t2\tMIT\n', encoding='utf-8', newline='\n')
            cases.write_text('# cloudleaf-search-resource-cases-v1\nid\tfamily\tinput\tkind\nempty\tf0\t\tempty\nlong\tf1\taaaa\tbudget\nbad\tf2\ta,\tinvalid\n', encoding='utf-8', newline='\n')
            for method in ['reference', 'exact', 'position', 'beam']:
                args = [str(runtime), str(cases), method, '1', '3', '1', '16384', '10', '50000', '5']
                observed = json.loads(subprocess.check_output([str(binary), *args], cwd=ROOT))
                old = json.loads(subprocess.check_output([str(original), *args], cwd=ROOT))
                case_rows = [{key: row[key] for key in ['id', 'input', 'family', 'kind']} for row in observed['cases']]
                summary = MODULE.validate_run(observed, case_rows, 3, 1)
                self.assertEqual(summary['cases'], 3)
                self.assertEqual(summary['outcomes']['rejected'], 1)
                self.assertTrue(all(row['all_observed_fields_equal'] for row in MODULE.validate_historical(observed, old)))

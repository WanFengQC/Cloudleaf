"""对真实保留短迹和小型数学序列验证离线报告；不构造长期验收。"""
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('soak_report', ROOT / 'scripts/report_windows_soak.py')
REPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPORT)
EVIDENCE = ROOT / 'docs/reports/cl007/round7'


class SoakReportTests(unittest.TestCase):
    def test_actual_short_evidence_reconstructs_original_stdout_and_never_accepts_long_test(self):
        report = REPORT.report_run(EVIDENCE / 'complete')
        segment = report['segments'][0]
        self.assertTrue(segment['evidence_integrity_verified'], segment['issues'])
        self.assertEqual(segment['resources']['samples'], 39)
        self.assertEqual(segment['latency']['events'], 480)
        self.assertEqual(segment['checkpoint_records'], 42)
        self.assertIsNone(segment['resources']['resource_gate_passed'])
        self.assertFalse(segment['candidate_contiguous_long_record_for_external_review'])
        self.assertFalse(report['long_term_production_reliability_accepted'])
        stdout = segment['files']['stdout.txt']
        self.assertEqual(stdout['resolved_original_sha256'], '508bf70ce3f50126e8f217957da3be5d6e1974ee965f88ac81841e95abbedbd9')

    def test_actual_interruption_and_resume_stay_separate_with_partial_denominator(self):
        report = REPORT.report_run(EVIDENCE / 'resumed')
        first, second = report['segments']
        self.assertTrue(first['evidence_integrity_verified'], first['issues'])
        self.assertTrue(second['evidence_integrity_verified'], second['issues'])
        self.assertEqual([row['latency']['events'] for row in report['segments']], [132, 480])
        self.assertTrue(first['interrupted'])
        self.assertIsNone(first['recovery']['success_rate'])
        self.assertFalse(first['latency']['trace_complete'])
        self.assertTrue(second['latency']['trace_complete'])
        self.assertFalse(report['segments_combined_to_claim_duration'])
        self.assertTrue(all(not row['candidate_contiguous_long_record_for_external_review'] for row in report['segments']))

    def test_resource_slope_units_and_single_sample_are_descriptive_only(self):
        rows = [{'elapsed_seconds': seconds, **{name: 10 + index * 2 for name in REPORT.RESOURCES}}
                for index, seconds in enumerate([0, 60, 120])]
        result = REPORT.resource_trend(rows)
        self.assertEqual(result['sample_span_seconds'], 120)
        for value in result['resources'].values():
            self.assertEqual(value['delta_last_minus_first'], 4)
            self.assertAlmostEqual(value['ols_units_per_hour'], 120)
        one = REPORT.resource_trend(rows[:1])
        self.assertIsNone(one['resources']['handles']['ols_units_per_hour'])
        self.assertIsNone(result['resource_gate_passed'])

    def test_changed_sample_bytes_and_nonmonotonic_times_remain_invalid(self):
        with tempfile.TemporaryDirectory() as temporary:
            copied = Path(temporary) / 'run'
            shutil.copytree(EVIDENCE / 'complete', copied)
            path = copied / 'segment-0001/samples.jsonl'
            rows = [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]
            rows[1]['elapsed_seconds'] = rows[0]['elapsed_seconds']
            path.write_text(''.join(json.dumps(row) + '\n' for row in rows), encoding='utf-8', newline='\n')
            result = REPORT.report_run(copied)['segments'][0]
            self.assertFalse(result['evidence_integrity_verified'])
            self.assertIsNone(result['resources'])
            self.assertTrue(any('SHA' in issue for issue in result['issues']))
            self.assertTrue(any('时间' in issue for issue in result['issues']))
            self.assertEqual(result['raw_sample_records'], 39)
            self.assertFalse(result['candidate_contiguous_long_record_for_external_review'])

    def test_native_duration_comes_from_raw_final_not_a_changed_summary_snapshot(self):
        with tempfile.TemporaryDirectory() as temporary:
            copied = Path(temporary) / 'run'
            shutil.copytree(EVIDENCE / 'complete', copied)
            path = copied / 'segment-0001/summary.json'
            value = json.loads(path.read_text(encoding='utf-8'))
            value['latest_events']['r5-pinned-availability']['steady_elapsed_ns'] = 10_000_000_000
            path.write_text(json.dumps(value), encoding='utf-8')
            result = REPORT.report_run(copied)['segments'][0]
            self.assertLess(result['native_steady_seconds'], 5)
            self.assertFalse(result['evidence_integrity_verified'])
            self.assertTrue(any('final' in issue for issue in result['issues']))


if __name__ == '__main__':
    unittest.main()

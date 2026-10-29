"""机械固定字节测试只读分析实现，不生成语言gold或生产频率。"""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from tools.data_pipeline.ngram_analysis import analyze

ROOT = Path(__file__).resolve().parents[1]


def inputs(folder, text, totals, corrupt=False):
    source = folder / 'input.gz'
    raw = bytearray(gzip.compress(text.encode('utf-8'), mtime=0))
    if corrupt:
        raw[-8] ^= 1
    source.write_bytes(raw)
    total = folder / 'totalcounts'
    total.write_text(totals, encoding='utf-8', newline='\n')
    return source, total


def inspect(source, total):
    return analyze(source, total,
                   expected_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                   expected_totalcounts_sha256=hashlib.sha256(total.read_bytes()).hexdigest())


class NgramAnalysisTests(unittest.TestCase):
    def test_complete_v3_and_shape_groups_are_descriptive_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, total = inputs(Path(temporary), '甲\t2000,10,2\t2001,4,1\n甲_NOUN\t2000,8,2\n',
                                   '2000,100,30,5 2001,50,20,3 2002,40,10,2\n')
            result = inspect(source, total)
            self.assertTrue(result['scan_completed'])
            self.assertTrue(result['integrity']['gzip_crc_verified'])
            self.assertEqual(result['counts']['rows'], 2)
            self.assertEqual(result['counts']['valid_triplets'], 3)
            self.assertEqual(result['structural_missing_token_year_pairs'], 3)
            self.assertEqual(result['totalcounts_years_without_export_groups'], [2002])
            self.assertEqual(result['year_fields']['2000']['field_2_sum'], 18)
            self.assertEqual(result['shape_rows']['tagged_suffix_shape'], 1)
            self.assertEqual(result['anomaly_counts'], {})
            self.assertEqual(result['semantics']['v3_field_meaning_status'], 'inferred-not-production-contract')
            self.assertFalse(result['production_approved'])
            self.assertTrue(all(value is None for value in result['formal_frequency'].values()))

    def test_all_anomalies_keep_later_rows_and_full_denominators(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, total = inputs(Path(temporary),
                '甲\t2001,2,3\t2001,4,1\t2000,101,6\tbad\t2003,5,1\n甲\t2000,2,1\n乙\t2000,7,1\n',
                '2000,100,30,5 2001,50,20,3\n')
            result = inspect(source, total)
            self.assertTrue(result['scan_completed'])
            self.assertEqual(result['counts']['rows'], 3)
            self.assertEqual(result['counts']['valid_triplets'], 6)
            for key in ['duplicate_token', 'duplicate_year', 'nonmonotonic_year', 'malformed_triplet',
                        'field_3_exceeds_field_2', 'year_missing_from_totalcounts', 'field_2_exceeds_total', 'field_3_exceeds_total']:
                self.assertEqual(result['anomaly_counts'][key], 1, key)
            self.assertEqual(result['year_fields']['2000']['field_2_sum'], 110)
            self.assertEqual(result['status'], 'complete-with-anomalies')

    def test_totals_bad_records_and_non_ascii_or_negative_numbers_are_counted(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, total = inputs(Path(temporary), '甲\t2000,-1,2\t2000,１,1\t2000,3,1\n',
                                   '2000,100,20,5 2000,200,30,10 broken 2001,1,x,2\n')
            result = inspect(source, total)
            self.assertTrue(result['scan_completed'])
            self.assertEqual(result['counts']['totalcounts_records'], 4)
            self.assertEqual(result['counts']['valid_triplets'], 1)
            self.assertEqual(result['anomaly_counts']['invalid_triplet_number'], 2)
            self.assertEqual(result['anomaly_counts']['duplicate_totalcounts_year'], 1)
            self.assertEqual(result['anomaly_counts']['malformed_totalcounts_record'], 1)
            self.assertEqual(result['anomaly_counts']['invalid_totalcounts_number'], 1)
            self.assertEqual(result['totalcounts_fields']['2000'], [100, 20, 5])

    def test_examples_are_limited_but_total_errors_are_not(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, total = inputs(Path(temporary), ''.join(f'词{i}\tbad\n' for i in range(20)), '2000,100,20,5\n')
            result = inspect(source, total)
            self.assertEqual(result['anomaly_counts']['malformed_triplet'], 20)
            self.assertEqual(len(result['anomaly_examples']['malformed_triplet']), 5)
            self.assertEqual(result['counts']['rows'], 20)

    def test_wrong_identity_blocks_scan_and_corrupt_crc_is_partial(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, total = inputs(Path(temporary), '甲\t2000,3,1\n', '2000,100,20,5\n', corrupt=True)
            result = inspect(source, total)
            self.assertFalse(result['scan_completed'])
            self.assertFalse(result['integrity']['gzip_crc_verified'])
            self.assertEqual(result['status'], 'partial')
            self.assertIsNotNone(result['fatal_error'])
            blocked = analyze(source, total, expected_source_sha256='0' * 64,
                              expected_totalcounts_sha256=hashlib.sha256(total.read_bytes()).hexdigest())
            self.assertEqual(blocked['status'], 'blocked-identity')
            self.assertEqual(blocked['counts']['rows'], 0)

    def test_cli_never_overwrites_report_and_pins_actual_r8_data(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            source, total = inputs(folder, '甲\t2000,3,1\n', '2000,100,20,5\n')
            output = folder / 'report.json'
            command = [sys.executable, '-X', 'utf8', '-m', 'tools.data_pipeline.ngram_analysis',
                       '--source', str(source), '--totalcounts', str(total), '--output', str(output)]
            result = subprocess.run(command, cwd=ROOT, capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(json.loads(output.read_bytes())['status'], 'blocked-identity')
            original = output.read_bytes()
            result = subprocess.run(command, cwd=ROOT, capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(output.read_bytes(), original)


if __name__ == '__main__':
    unittest.main()

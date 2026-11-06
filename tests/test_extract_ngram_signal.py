"""测试实际研发提取函数，不复制其统计逻辑。"""
import importlib.util
import io
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location('extract_ngram_signal', Path(__file__).resolve().parents[1] / 'scripts/extract_ngram_signal.py')
SIGNAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SIGNAL)


class NgramSignalTests(unittest.TestCase):
    def test_exact_word_window_and_zero_are_distinct_from_missing(self):
        source = '我\t1999,1000,9\t2000,7,3\t2019,11,5\t2020,5000,4\n我_NOUN\t2000,300,30\n握\t2000,0,0\n'.encode('utf-8')
        observed, _, rows, groups = SIGNAL.scan_observations(io.BytesIO(source), {'我', '握', '缺失'})
        self.assertEqual(observed, {'我': {'raw_match_sum': 18, 'year_groups': 2}, '握': {'raw_match_sum': 0, 'year_groups': 1}})
        self.assertEqual((rows, groups), (3, 6))
        self.assertNotIn('缺失', observed)
        self.assertNotIn('我_NOUN', observed)

    def test_duplicate_word_or_year_is_rejected_without_double_count(self):
        for source in ['我\t2000,1,1\n我\t2001,2,1\n', '我\t2000,1,1\t2000,2,1\n']:
            with self.subTest(source=source), self.assertRaises(ValueError):
                SIGNAL.scan_observations(io.BytesIO(source.encode('utf-8')), {'我'})

    def test_malformed_unmatched_records_still_fail_in_raw_denominator(self):
        for source in ['其他\t2000,1\n', '其他\t2000,-1,2\n', '其他\t２０００,1,2\n', '其他\n']:
            with self.subTest(source=source), self.assertRaises(ValueError):
                SIGNAL.scan_observations(io.BytesIO(source.encode('utf-8')), {'我'})

    def test_invalid_utf8_is_rejected(self):
        with self.assertRaises(UnicodeDecodeError):
            SIGNAL.scan_observations(io.BytesIO(b'\xff\t2000,1,1\n'), {'我'})


if __name__ == '__main__':
    unittest.main()

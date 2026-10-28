"""真实工具的自编源字段回归，不能把夹具当正式双语数据。"""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from tools.data_pipeline.source_assertions import build


class SourceAssertionTests(unittest.TestCase):
    def fixture(self, folder):
        source = Path(folder) / 'source.jsonl'
        rows = [{'word': '夹具', 'lang_code': 'zh', 'pos': 'noun',
                 'senses': [{'glosses': ['自编义项']}],
                 'translations': [{'lang_code': 'en', 'word': 'fixture', 'sense': '原始未对齐标签'}]},
                {'word': '隔离', 'lang_code': 'zh', 'pos': 'noun', 'copyright': 'not reviewed',
                 'senses': [{'glosses': ['不可导出']}]}]
        source.write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in rows), encoding='utf-8', newline='\n')
        lock = {'source_id': 'synthetic-fixture', 'version': 'fixture-1', 'research_only': True,
                'review_status': 'approved', 'approved_scope': 'Bounded research',
                'format': 'wiktionary-zh-prefix-jsonl', 'bytes': source.stat().st_size,
                'sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'line_count': 2,
                'license_id': 'MIT', 'license_sha256': 'a' * 64,
                'attribution': 'Synthetic mechanism fixture only'}
        return source, lock

    def test_source_pointer_and_reverse_lookup_keep_one_directed_fact(self):
        with tempfile.TemporaryDirectory() as folder:
            source, lock = self.fixture(folder)
            first = Path(folder) / 'a'
            result = build(source, lock, first)
            assertions = [json.loads(line) for line in (first / 'assertions.jsonl').read_text(encoding='utf-8').splitlines()]
            self.assertEqual(len(assertions), 1)
            row = assertions[0]
            self.assertEqual(row['source']['line_number'], 1)
            self.assertEqual(row['translations'][0]['source_pointer'], '/translations/0')
            self.assertIsNone(row['translations'][0]['sense_id'])
            self.assertEqual(row['translations'][0]['source_sense_label'], '原始未对齐标签')
            self.assertEqual(row['senses'][0]['source_pointer'], '/senses/0')
            index = json.loads((first / 'lookup.json').read_text(encoding='utf-8'))
            self.assertEqual(index['zh_to_en']['夹具'], index['en_to_zh']['fixture'])
            self.assertEqual(result['quarantined_entries'], 1)
            self.assertTrue(result['research_only'])
            self.assertFalse(result['production_language_validated'])
            second = Path(folder) / 'b'
            build(source, lock, second)
            self.assertEqual({p.name:p.read_bytes() for p in first.iterdir()}, {p.name:p.read_bytes() for p in second.iterdir()})

    def test_source_hash_or_research_scope_cannot_be_silently_changed(self):
        with tempfile.TemporaryDirectory() as folder:
            source, lock = self.fixture(folder)
            source.write_bytes(source.read_bytes() + b'\n')
            with self.assertRaises(ValueError):
                build(source, lock, Path(folder) / 'bad')
            lock['research_only'] = False
            with self.assertRaises(ValueError):
                build(source, lock, Path(folder) / 'prod')

"""实际锁和审查快照的机械一致性，不代替许可真实性审核。"""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from tools.data_pipeline.source_review import build_matrix

ROOT = Path(__file__).resolve().parents[1]


def copy_inputs(folder):
    for name in ['data/sources', 'data/licenses']:
        shutil.copytree(ROOT / name, folder / name)
    target = folder / 'docs/reports/cl009/round6'
    target.mkdir(parents=True)
    for name in ['wikipedia-gate.json', 'wikipedia-acquisition.json']:
        shutil.copyfile(ROOT / 'docs/reports/cl009/round6' / name, target / name)


class SourceReviewTests(unittest.TestCase):
    def test_actual_matrix_preserves_approval_scope_and_unknowns(self):
        result = build_matrix(ROOT)
        self.assertEqual(len(result['sources']), 9)
        rows = {row['id']: row for row in result['sources']}
        mirror = rows['cccedict:mirror-20260604']
        self.assertEqual(mirror['project_review_status'], 'approved-licensed-fixed-mirror')
        self.assertTrue(mirror['license_text_verified'])
        self.assertTrue(mirror['permissions']['commercial_use'])
        self.assertFalse(mirror['research_only'])
        self.assertTrue(rows['wiktionary:zh-20261001-prefix8388608']['research_only'])
        self.assertIsNone(rows['subtlex-ch']['permissions']['commercial_use'])
        self.assertIsNone(rows['subtlex-ch']['approved_scope'])
        self.assertIsNone(rows['google-books-ngrams-zh2019']['original_sha256'])
        self.assertFalse(rows['wikipedia:zh-page342-rev94816134']['statistics_ready'])
        self.assertFalse(result['creates_new_source_approval'])

    def test_changed_license_or_registered_archive_is_rejected(self):
        for change in ['license', 'archive']:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                copy_inputs(root)
                if change == 'license':
                    (root / 'data/licenses/CC-BY-SA-4.0.txt').write_text('changed', encoding='utf-8')
                else:
                    path = root / 'data/sources/source-review.jsonl'
                    values = [json.loads(row) for row in path.read_text(encoding='utf-8').splitlines()]
                    values[-1]['archive_sha256'] = 'a' * 64
                    path.write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in values), encoding='utf-8')
                with self.assertRaises(ValueError):
                    build_matrix(root)

    def test_duplicate_registry_or_unregistered_lock_is_rejected(self):
        for change in ['duplicate', 'unregistered']:
            with self.subTest(change=change), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                copy_inputs(root)
                if change == 'duplicate':
                    path = root / 'data/sources/source-review.jsonl'
                    original = path.read_text(encoding='utf-8')
                    path.write_text(original + original.splitlines()[0] + '\n', encoding='utf-8')
                else:
                    path = root / 'data/sources/locks/unihan-17.0.0.json'
                    value = json.loads(path.read_text(encoding='utf-8'))
                    value['source_id'] = 'unreviewed-source'
                    path.write_text(json.dumps(value), encoding='utf-8')
                with self.assertRaises(ValueError):
                    build_matrix(root)

    def test_cli_double_build_is_identical_and_never_overwrites(self):
        with tempfile.TemporaryDirectory() as folder:
            paths = [Path(folder) / name for name in ['a.json', 'b.json']]
            for path in paths:
                subprocess.run([sys.executable, '-X', 'utf8', '-m', 'tools.data_pipeline.source_review',
                                '--root', str(ROOT), '--output', str(path)], cwd=ROOT,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
            self.assertEqual(paths[0].read_bytes(), paths[1].read_bytes())
            original = paths[0].read_bytes()
            result = subprocess.run([sys.executable, '-X', 'utf8', '-m', 'tools.data_pipeline.source_review',
                                     '--root', str(ROOT), '--output', str(paths[0])], cwd=ROOT,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(paths[0].read_bytes(), original)

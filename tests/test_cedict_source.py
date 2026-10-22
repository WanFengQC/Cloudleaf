"""自编CC-CEDICT容器夹具，仅测试导入与失败边界，不作为语言gold。"""

from pathlib import Path
import json
import shutil
import tempfile
import unittest
from zipfile import ZipFile, ZipInfo

from tools.data_pipeline.pipeline import build, canonical, digest, read_json, verify_release, write_json
from tools.data_pipeline.cedict_source import source_key


class CedictSourceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'input.zip'
        self.license = self.root / 'LICENSE.txt'
        self.license.write_text('自编夹具，不是正式数据许可\n', encoding='utf-8')
        self.lock_path = self.root / 'lock.json'

    def prepare(self, entries, *, member='cedict_ts.u8'):
        header = {'version':'1','subversion':'0','format':'ts','charset':'UTF-8','entries':str(len(entries)),
                  'publisher':'MDBG','license':'https://creativecommons.org/licenses/by-sa/4.0/','date':'fixture'}
        content = ''.join('#! ' + key + '=' + value + '\n' for key, value in header.items())
        content += '\n'.join(entries) + '\n'
        data = content.encode('utf-8')
        with ZipFile(self.source, 'w') as archive:
            archive.writestr(member, data)
        self.lock = {'schema_version':1,'review_status':'approved','format':'cedict-zip-v1','zip_full_crc_verified':True,
                     'source_id':'cloudleaf:test-cedict-archive','version':'fixture-v1','license_id':'MIT',
                     'sha256':digest(self.source.read_bytes()),'license_sha256':digest(self.license.read_bytes()),
                     'source_member':'cedict_ts.u8','expanded_bytes':len(data),'expanded_sha256':digest(data),
                     'source_header':header,'limitations':['仅自编夹具'], 'attribution':'Cloudleaf fixture',
                     'url':'https://example.invalid/fixed.zip','license_url':'https://example.invalid/license',
                     'transformations':'test-only','automated_download_allowed':False}
        write_json(self.lock_path, self.lock)

    def create(self, name='release'):
        target = self.root / name
        build(self.lock_path, self.source, self.license, target)
        verify_release(target)
        return target

    def test_complete_archive_unknown_language_fields_and_different_local_names(self):
        self.prepare(['你好 你好 [ni3 hao3] /hello/hi/', '女 女 [nu:3] /woman/'])
        first = self.create('first')
        renamed = self.root / 'different-name.zip'
        shutil.copyfile(self.source, renamed)
        self.source = renamed
        second = self.create('second')
        self.assertEqual({p.name:p.read_bytes() for p in first.iterdir()}, {p.name:p.read_bytes() for p in second.iterdir()})
        rows = [json.loads(line) for line in (first / 'records.jsonl').read_text(encoding='utf-8').splitlines()]
        self.assertTrue(all(row['frequency'] is None and row['pos'] is None for row in rows))
        self.assertTrue(all(row['evidence'][0]['member'] == 'cedict_ts.u8' for row in rows))
        self.assertIn(['nv'], [row['pinyin'] for row in rows])

    def test_meaning_correction_does_not_change_source_identity_but_reading_case_does(self):
        self.assertEqual(source_key('你好','你好','ni3 hao3'), source_key('你好','你好','ni3 hao3'))
        self.assertNotEqual(source_key('王','王','Wang2'), source_key('王','王','wang2'))
        self.prepare(['你好 你好 [ni3 hao3] /hello/'])
        first = json.loads((self.create('first') / 'records.jsonl').read_text(encoding='utf-8'))
        self.prepare(['你好 你好 [ni3 hao3] /greeting/'])
        second = json.loads((self.create('second') / 'records.jsonl').read_text(encoding='utf-8'))
        self.assertEqual(first['id'], second['id'])
        self.assertNotEqual(first['raw_senses'], second['raw_senses'])

    def test_duplicate_merges_evidence_conflict_isolates_every_version(self):
        self.prepare(['你好 你好 [ni3 hao3] /hello/', '你好 你好 [ni3 hao3] /hello/',
                      '王 王 [wang2] /king/', '王 王 [wang2] /changed/', '女 女 [nu:3] /woman/'])
        output = self.create()
        counts = read_json(output / 'quality.json')['counts']
        self.assertEqual(counts['accepted_records'], 2)
        self.assertEqual(counts['duplicate_records'], 1)
        self.assertEqual(counts['conflicting_lexemes'], 1)
        self.assertEqual(counts['rejected_records'], 2)
        rows = [json.loads(line) for line in (output / 'records.jsonl').read_text(encoding='utf-8').splitlines()]
        self.assertEqual(len(next(row for row in rows if row['text'] == '你好')['evidence']), 2)

    def test_unsupported_and_budget_are_separate_from_bad_source_rows(self):
        self.prepare(['你好 你好 [ni3 hao3] /hello/', '花兒 花儿 [hua1 r5] /flower/',
                      '標記 标记 [xx5] /mark/', '符號 符号 [ABC] /symbols/',
                      '長詞 长词 [' + ' '.join(['chang2'] * 17) + '] /long/', 'bad'])
        q = read_json(self.create() / 'quality.json')
        self.assertEqual(q['counts']['unsupported_runtime_readings'], 3)
        self.assertEqual(q['counts']['runtime_budget_isolations'], 1)
        self.assertEqual(q['counts']['malformed_or_conflicting_records'], 1)

    def test_header_count_plain_hash_and_archive_paths_fail_closed(self):
        for change in ['count','hash','path','extra','symlink','fifo','socket','character','block']:
            with self.subTest(change=change):
                self.prepare(['你好 你好 [ni3 hao3] /hello/'])
                if change == 'count':
                    self.lock['source_header']['entries'] = '2'
                elif change == 'hash':
                    self.lock['expanded_sha256'] = 'a' * 64
                else:
                    with ZipFile(self.source) as archive:
                        data = archive.read('cedict_ts.u8')
                    with ZipFile(self.source, 'w') as archive:
                        if change == 'path':
                            archive.writestr('../cedict_ts.u8', data)
                        elif change == 'extra':
                            archive.writestr('cedict_ts.u8', data)
                            archive.writestr('other', b'x')
                        else:
                            info = ZipInfo('cedict_ts.u8')
                            kinds = {'symlink':0o120000, 'fifo':0o010000, 'socket':0o140000,
                                     'character':0o020000, 'block':0o060000}
                            info.external_attr = (kinds[change] | 0o777) << 16
                            archive.writestr(info, data)
                    self.lock['sha256'] = digest(self.source.read_bytes())
                write_json(self.lock_path, self.lock)
                with self.assertRaises(ValueError):
                    self.create(change)
                self.assertFalse((self.root / change).exists())

    def test_derived_quality_and_source_identity_cannot_be_forged_with_new_hashes(self):
        self.prepare(['你好 你好 [ni3 hao3] /hello/'])
        output = self.create()
        quality = read_json(output / 'quality.json')
        quality['counts']['accepted_multicharacter_records'] = 0
        write_json(output / 'quality.json', quality)
        manifest = read_json(output / 'manifest.json')
        data = (output / 'quality.json').read_bytes()
        manifest['resources']['quality.json'] = {'sha256':digest(data),'bytes':len(data)}
        write_json(output / 'manifest.json', manifest)
        with self.assertRaisesRegex(ValueError, '派生计数'):
            verify_release(output)

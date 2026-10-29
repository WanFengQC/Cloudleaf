"""从固定获批词条原件生成真实待审队列，不生成新语义或gold。"""
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path.cwd()
OUT = ROOT / 'docs/acceptance/cl009-round8'
EVIDENCE = ROOT / '.local/round8-materials'
EVIDENCE.mkdir(exist_ok=False)
PACKAGE = ROOT / '.local/round5-clean-6d3d4e1/release-a'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')


manifest_raw = (PACKAGE / 'manifest.json').read_bytes()
assert sha(manifest_raw) == '5971456293a4e868ae6372d9d928f3ad9edef4429ec0ba233eba8a5424ba77d5'
manifest = json.loads(manifest_raw)
old_files = {'manifest.json': sha(manifest_raw)}
for name, expected in manifest['resources'].items():
    actual = PACKAGE / name
    assert actual.stat().st_size == expected['bytes'] and file_sha(actual) == expected['sha256'], name
    old_files[name] = expected['sha256']
lock_raw = (ROOT / 'data/sources/locks/cccedict-20260604-mirror7019e1c.json').read_bytes()
lock = json.loads(lock_raw)
archive = ROOT / '.local/round4-source-cedict/cccedict-20260604-mirror7019e1c.zip'
assert file_sha(archive) == lock['sha256'] and archive.stat().st_size == lock['bytes']
with zipfile.ZipFile(archive) as package:
    assert package.testzip() is None
    member = package.read(lock['source_member'])
assert sha(member) == lock['expanded_sha256'] and len(member) == lock['expanded_bytes']
lines = member.decode('utf-8').splitlines()
requests = []
with (PACKAGE / 'records.jsonl').open('rb') as source:
    for _ in range(20):
        raw = next(source)
        value = json.loads(raw)
        assert value['source'] == lock['source_id'] and value['frequency'] is None and value['frequency_kind'] == 'unknown'
        source_rows = []
        for item in value['evidence']:
            assert item['member'] == lock['source_member'] and lines[item['line'] - 1] == item['raw']
            source_rows.append({'member': item['member'], 'line': item['line'], 'raw': item['raw'],
                                'raw_utf8_without_line_ending_sha256': sha(item['raw'].encode('utf-8'))})
        requests.append({'format': 'cloudleaf-semantic-review-request-v1', 'request_id': 'r8-semantic:' + value['id'],
                         'status': 'pending', 'purpose': 'dictionary-semantic-source-review-not-IME-gold',
                         'entry_id': value['id'], 'source_record_id': value['source_record_id'],
                         'source_id': value['source'], 'source_version': value['source_version'],
                         'source_archive_sha256': lock['sha256'], 'source_member_sha256': lock['expanded_sha256'],
                         'source_lock_sha256': sha(lock_raw), 'facts_sha256': manifest['resources']['records.jsonl']['sha256'],
                         'facts_jsonl_record_with_LF_sha256': sha(raw), 'text': value['text'],
                         'traditional': value['traditional'], 'simplified': value['simplified'],
                         'raw_reading': value['raw_reading'], 'raw_senses': value['raw_senses'], 'evidence': source_rows,
                         'license_id': value['license_id'], 'license_file': 'LICENSE.txt',
                         'source_url': lock['source_page'], 'attribution': lock['attribution'],
                         'pos': None, 'reviewed_senses': None, 'directed_translations': None, 'phrases': None,
                         'frequency': None, 'frequency_kind': 'unknown', 'annotation_author_id': None,
                         'reviewers': [], 'adjudication': None, 'publication_eligible': False})
assert [row['entry_id'] for row in requests] == sorted(row['entry_id'] for row in requests)
target = OUT / 'semantic-review-queue.jsonl'
assert not target.exists()
target.write_bytes(b''.join((json.dumps(row, ensure_ascii=False, sort_keys=True, separators=(',', ':')) + '\n').encode('utf-8') for row in requests))
(OUT / 'LICENSE.txt').write_bytes((PACKAGE / 'LICENSE.txt').read_bytes())
notice = (PACKAGE / 'NOTICE.txt').read_text(encoding='utf-8').rstrip() + '\n\n'
notice += ('Cloudleaf Round 8 modification: select the first 20 stable entry IDs from the fixed reviewed fact package, '
           'preserve original readings, raw English senses and source evidence, and organize pending human semantic review requests. '
           'No POS, aligned senses, inverse translation, frequency, reviewer decision or IME gold is inferred. '
           'This derived queue remains CC BY-SA 4.0; Cloudleaf tool code is separately MIT.\n')
(OUT / 'NOTICE.txt').write_text(notice, encoding='utf-8', newline='\n')
unihan = ROOT / '.local/data-sources/Unihan-17.0.0.zip'
unihan_lock = json.loads((ROOT / 'data/sources/locks/unihan-17.0.0.json').read_text(encoding='utf-8'))
assert file_sha(unihan) == unihan_lock['sha256']
docs = ROOT / '.local/round8-source-documents'
acquisition = json.loads((docs / 'acquisition.json').read_text(encoding='utf-8')) + json.loads((docs / 'details-acquisition.json').read_text(encoding='utf-8'))
for row in acquisition:
    suffix = '.html' if row['id'] in ['info', 'datasets'] else '.utf8'
    assert file_sha(docs / (row['id'] + suffix)) == row['sha256']
headers = json.loads((docs / 'data-head-only.json').read_text(encoding='utf-8'))
assert all(row['method'] == 'HEAD' and row['body_bytes_read'] == 0 and row['sha256'] is None for row in headers)
save(OUT / 'google-source-evidence.json', {'schema_version': 1, 'source_id': 'google-books-ngrams-zh2019',
     'review_status': 'bulk-license-evidence-located-data-not-acquired', 'production_approved': False,
     'corpus_version': 'googlebooks-chi-sim-20200217', 'license_id': 'CC-BY-3.0',
     'license_statement_source': 'https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html',
     'primary_documents': acquisition, 'data_HEAD_only': headers,
     'data_partitions_downloaded': False, 'token_frequency': None, 'document_frequency': None, 'zipf': None,
     'remaining': ['Fixed raw partition SHA and gzip CRC', 'Actual v3 line/denominator protocol',
                   '2019 cutoff resolution: FAQ book threshold versus v1/v2 landing occurrence wording',
                   'Extraction/versioned tokenizer and dataset period', 'Approved acquisition and production review',
                   'Independent representativeness and language evaluation']})
identity = {'baseline': '3d573384ce1f31e437bdb4e7dc109cf438c261f4', 'queue_requests': len(requests),
            'queue_sha256': file_sha(target), 'queue_purpose_is_IME_gold': False,
            'queue_all_pending': True, 'old_package_files_sha256': old_files,
            'cccedict_archive': {'path': str(archive), 'bytes': archive.stat().st_size, 'sha256': file_sha(archive)},
            'unihan_archive': {'path': str(unihan), 'bytes': unihan.stat().st_size, 'sha256': file_sha(unihan)},
            'generator_sha256': file_sha(Path(__file__)), 'source_materials_changed': False,
            'production_data_or_publisher_created': False}
save(EVIDENCE / 'identity.json', identity)
print(json.dumps({'requests': len(requests), 'queue_sha256': identity['queue_sha256'], 'all_pending': True,
                  'original_package_files_verified': len(old_files), 'google_data_body_downloaded': False}), flush=True)

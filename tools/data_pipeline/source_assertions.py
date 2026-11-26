"""仅导出批准研究范围中的源断言；英语反查不是反向翻译事实。"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from urllib.parse import quote

from .annotation_workflow import canonical, require
from .word_source import select_fields

ROOT = Path(__file__).resolve().parents[2]


def build(source, lock, output):
    require(lock.get('review_status') == 'approved' and lock.get('research_only') is True
            and lock.get('format') == 'wiktionary-zh-prefix-jsonl', '只允许已批准的固定研究片段')
    data = source.read_bytes()
    require(len(data) == lock['bytes'] and hashlib.sha256(data).hexdigest() == lock['sha256'], '原件字节/SHA不符')
    lines = data.splitlines(keepends=True)
    require(len(lines) == lock['line_count'], '原件行数不符')
    license_data = (ROOT / lock['license_file']).read_bytes()
    require(hashlib.sha256(license_data).hexdigest() == lock['license_sha256'], '许可原文SHA不符')
    require(not output.exists(), '输出已存在，禁止覆盖')
    records, quarantine, skipped, missing_pinyin = [], [], 0, 0
    zh_to_en, en_to_zh = {}, {}
    for number, raw in enumerate(lines, 1):
        row = json.loads(raw.decode('utf-8'))
        word = row.get('word', '')
        if (row.get('lang_code') != 'zh' or not isinstance(word, str) or not 2 <= len(word) <= 16
                or not all(0x3400 <= ord(c) <= 0x9FFF or 0x20000 <= ord(c) <= 0x3347F for c in word)):
            skipped += 1
            continue
        origin = {'source_id': lock['source_id'], 'version': lock['version'], 'line_number': number,
                  'line_sha256': hashlib.sha256(raw).hexdigest(),
                  'attribution_url': 'https://zh.wiktionary.org/w/index.php?title=' + quote(word) + '&action=history'}
        try:
            fields = select_fields(row)
            require(not fields['rights_markers'], 'unreviewed_additional_rights_marker')
        except ValueError as error:
            quarantine.append({'source': origin, 'reason': str(error)})
            continue
        if not fields['mandarin_pinyin']:
            missing_pinyin += 1
            continue
        identity = lock['source_id'] + ':' + str(number)
        senses = [{'id': identity + ':sense:' + str(index), 'source_pointer': '/senses/' + str(index),
                   'glosses': sense.get('glosses', []), 'tags': sense.get('tags', [])}
                  for index, sense in enumerate(fields['senses'])]
        translations = []
        for index, item in enumerate(row.get('translations', [])):
            if not isinstance(item, dict) or item.get('lang_code') != 'en' or not isinstance(item.get('word'), str) or not item['word'].strip():
                continue
            translation_id = identity + ':translation:' + str(index)
            translations.append({'id': translation_id, 'source_pointer': '/translations/' + str(index),
                                 'from_language': 'zh', 'to_language': 'en', 'word': item['word'],
                                 'source_sense_label': item.get('sense'), 'sense_id': None,
                                 'tags': item.get('tags', []), 'raw_tags': item.get('raw_tags', [])})
            zh_to_en.setdefault(word, []).append(translation_id)
            en_to_zh.setdefault(item['word'], []).append(translation_id)
        records.append({'id': identity, 'word': word, 'source': origin, 'pos': fields['pos'],
                        'pos_title': fields['pos_title'], 'pos_source_pointer': '/pos', 'senses': senses,
                        'mandarin_pinyin': fields['mandarin_pinyin'], 'translations': translations,
                        'frequency': None, 'reviewed_language_fact': False})
    output.mkdir(parents=True)
    (output / 'assertions.jsonl').write_bytes(b''.join(canonical(row) for row in records))
    (output / 'quarantined.jsonl').write_bytes(b''.join(canonical(row) for row in quarantine))
    (output / 'lookup.json').write_bytes(canonical({'zh_to_en': zh_to_en, 'en_to_zh': en_to_zh,
                                                  'reverse_lookup_creates_reverse_assertions': False}))
    (output / 'upstream-documents.json').write_bytes(canonical({'license_id': lock['license_id'],
        'license_sha256': lock['license_sha256'], 'license_utf8': license_data.decode('utf-8'),
        'attribution': lock['attribution'], 'source_lock': lock,
        'modifications': 'Selected source assertions, stable snapshot-position IDs and lookup indexes; no frequency or reverse-equivalence inference.'}))
    result = {'format': 'cloudleaf-source-assertions-v1', 'research_only': True, 'production_language_validated': False,
              'source_sha256': lock['sha256'], 'source_lock_canonical_sha256': hashlib.sha256(canonical(lock)).hexdigest(),
              'source_lines': len(lines), 'entries': len(records), 'senses': sum(len(row['senses']) for row in records),
              'translations': sum(len(row['translations']) for row in records), 'quarantined_entries': len(quarantine),
              'outside_scope': skipped, 'missing_explicit_pinyin': missing_pinyin,
              'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'builder_files_sha256': {name: hashlib.sha256((Path(__file__).parent / name).read_bytes()).hexdigest()
                                      for name in ['source_assertions.py', 'annotation_workflow.py', 'word_source.py', 'pipeline.py']},
              'files_sha256': {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(output.iterdir())},
              'limitations': ['固定片段未验证完整gzip/CRC或所有页面例外；原始义项/翻译标签未人工审读或对齐，不是正式双语词库。']}
    (output / 'manifest.json').write_bytes(canonical(result))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--lock', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.source, json.loads(args.lock.read_text(encoding='utf-8')), args.output_dir), ensure_ascii=False))


if __name__ == '__main__':
    main()

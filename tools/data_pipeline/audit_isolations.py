"""只读核对完整CC-CEDICT隔离事实；不改变生产音节或从形状推断语言规范。"""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import re
import shutil
import unicodedata

from .cedict_source import read_archive
from .pipeline import SYLLABLES, builder_fingerprint, canonical, digest, load_lock, verify_release

TOKEN = re.compile(r'[A-Za-züÜ:]+[1-5]\Z')
ENTRY = re.compile(r'(?P<traditional>\S+) (?P<simplified>\S+) \[(?P<reading>[^\[\]]+)\] /(?P<senses>.+)/\Z')


def normalized(token: str) -> str:
    """独立按冻结格式重建无调音串；不调用生产normalize_reading。"""
    value = token.casefold().replace('u:', 'ü').replace('v', 'ü')
    if value[-1:] in '12345':
        value = value[:-1]
    value = ''.join(char for char in unicodedata.normalize('NFD', value)
                    if char not in {'\u0300', '\u0301', '\u0304', '\u030c'})
    value = unicodedata.normalize('NFC', value).replace('ü', 'v')
    if value[:2] in {'jv', 'qv', 'xv', 'yv'}:
        value = value[0] + 'u' + value[2:]
    return value


def classify_token(token: str) -> dict:
    norm = normalized(token) if TOKEN.fullmatch(token) else None
    if norm in SYLLABLES:
        category = 'supported'
    elif token.casefold() == 'r5':
        category = 'erhua_suffix'
    elif token.casefold() == 'xx5':
        category = 'unspecified_reading'
    elif TOKEN.fullmatch(token) and norm in {'m', 'n', 'ng', 'hm', 'hng'}:
        category = 'syllabic_nasal'
    elif TOKEN.fullmatch(token):
        category = 'tone_number_not_in_inventory'
    elif token == ',':
        category = 'separator_comma'
    elif token == '·':
        category = 'separator_middle_dot'
    elif token == ':':
        category = 'separator_colon'
    elif re.fullmatch(r'(?:[A-Za-züÜ:]+[1-5]){2,}', token):
        category = 'joined_tone_number_syllables'
    elif re.fullmatch(r'[0-9]+', token):
        category = 'literal_number'
    elif re.fullmatch(r'[A-Za-z]+', token):
        category = 'untoned_ascii_literal'
    else:
        category = 'other_source_token'
    return {'token': token, 'normalized': norm, 'category': category,
            'language_validity': 'not_independently_adjudicated',
            'evidence_rule': 'Frozen source expression and runtime inventory comparison; not a Mandarin standard ruling'}


def independent_reason(match: re.Match | None) -> str | None:
    if match is None:
        return 'cedict_syntax'
    tokens = match['reading'].split(' ')
    if not all(TOKEN.fullmatch(token) for token in tokens):
        return 'unsupported_cedict_reading'
    if any(normalized(token) not in SYLLABLES for token in tokens):
        return 'unsupported_reading'
    word = match['simplified']
    if (not word or word != word.strip() or len(word) > 64 or len(tokens) > 16
            or sum(len(normalized(token)) for token in tokens) > 64
            or any(unicodedata.category(char) in {'Cc', 'Cf', 'Cs'} for char in word)):
        return 'cedict_runtime_budget'
    # 完整原件当前没有此类坏义项；仍明确独立拒绝控制字符/空白错误。
    if any(not text or text != text.strip() or any(unicodedata.category(c) in {'Cc', 'Cf', 'Cs'} for c in text)
           for text in match['senses'].split('/')):
        return 'invalid_sense'
    return None


def audit_isolations(lock_path: Path, source: Path, release: Path) -> dict:
    lock = load_lock(lock_path)
    if lock['format'] != 'cedict-zip-v1' or digest(source.read_bytes()) != lock['sha256']:
        raise ValueError('审计只接受已锁定完整CC-CEDICT ZIP原件')
    manifest = verify_release(release)
    if manifest['source'] != lock:
        raise ValueError('原件锁与冻结release来源不同')
    text, _ = read_archive(source, lock)
    rejected_rows = [json.loads(line) for line in (release/'rejected.jsonl').read_text(encoding='utf-8').splitlines()]
    rejected = {row['line']: row for row in rejected_rows}
    if len(rejected) != len(rejected_rows):
        raise ValueError('重复隔离源行')
    accepted = {}
    for line in (release/'records.jsonl').read_text(encoding='utf-8').splitlines():
        record = json.loads(line)
        for evidence in record['evidence']:
            number = evidence['line']
            if number in accepted or number in rejected:
                raise ValueError('源行重复或同时接受/隔离')
            accepted[number] = evidence['raw']
    output, seen, reason_counts, token_counts = [], set(), Counter(), Counter()
    for number, raw in enumerate(text.splitlines(), 1):
        if not raw or raw.startswith('#'):
            continue
        seen.add(number)
        match = ENTRY.fullmatch(raw)
        reason = independent_reason(match)
        if reason is None:
            if accepted.get(number) != raw or number in rejected:
                raise ValueError(f'接受行独立重放不同：{number}')
            continue
        expected = {'member': lock['source_member'], 'line': number, 'raw': raw, 'reason': reason}
        if rejected.get(number) != expected or number in accepted:
            raise ValueError(f'隔离行独立重放不同：{number}')
        tokens = [] if match is None else [classify_token(token) for token in match['reading'].split(' ')]
        categories = sorted({token['category'] for token in tokens if token['category'] != 'supported'})
        token_counts.update(token['category'] for token in tokens if token['category'] != 'supported')
        classification = ('source_syntax' if reason == 'cedict_syntax' else
                          'source_senses' if reason == 'invalid_sense' else
                          'runtime_budget' if reason == 'cedict_runtime_budget' else
                          categories[0] if len(categories) == 1 else 'multiple_source_expression_gaps')
        reason_counts[reason] += 1
        output.append({'member': lock['source_member'], 'line': number, 'raw': raw,
                       'source_line_sha256': digest(raw.encode('utf-8')), 'original_reason': reason,
                       'traditional': match['traditional'] if match else None,
                       'simplified': match['simplified'] if match else None,
                       'raw_reading': match['reading'] if match else None,
                       'tokens': tokens, 'classification': classification,
                       'classification_dimensions': categories,
                       'language_validity': 'not_independently_adjudicated',
                       'reason_evidence': 'Independent frozen grammar/normalization/budget replay equals original rejection; source expression is not silently changed',
                       'action': 'retain_isolated_no_runtime_change'})
    if seen != set(accepted) | set(rejected) or len(output) != len(rejected):
        raise ValueError('原件与release分区缺失、越界或重复')
    return {'summary': {'schema_version': 1, 'source_entries': len(seen),
            'accepted_records': manifest['entry_count'], 'accepted_source_lines': len(accepted),
            'isolated_records': len(output), 'reason_counts': dict(sorted(reason_counts.items())),
            'unsupported_token_counts': dict(sorted(token_counts.items())),
            'record_classification_counts': dict(sorted(Counter(row['classification'] for row in output).items())),
            'source_sha256': lock['sha256'], 'source_lock_sha256': digest(lock_path.read_bytes()),
            'manifest_sha256': digest((release/'manifest.json').read_bytes()),
            'rejections_sha256': digest((release/'rejected.jsonl').read_bytes()),
            'runtime_sha256': digest((release/'runtime.tsv').read_bytes()),
            'builder_files_sha256': builder_fingerprint(), 'language_accuracy_valid': False,
            'source_language_standard_validity': 'not_independently_adjudicated',
            'scope': 'Read-only fixed zero-conflict source per-line classification; conflicts fail closed, not production parser expansion or Mandarin correctness judgment'},
            'records': output}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['lock', 'source', 'release', 'output-dir']:
        parser.add_argument('--' + flag, type=Path, required=True)
    args = parser.parse_args()
    if args.output_dir.exists():
        raise ValueError('审计不覆盖已有输出目录')
    result = audit_isolations(args.lock, args.source, args.release)
    args.output_dir.mkdir(parents=True, exist_ok=False)
    data = b''.join(canonical(row) for row in result['records'])
    (args.output_dir/'records.jsonl').write_bytes(data)
    for name in ['LICENSE.txt', 'NOTICE.txt']:
        shutil.copyfile(args.release/name, args.output_dir/name)
    result['summary']['output_sha256'] = {path.name:digest(path.read_bytes()) for path in args.output_dir.iterdir()}
    (args.output_dir/'summary.json').write_bytes(canonical(result['summary']))
    print(json.dumps({k:result['summary'][k] for k in ['source_entries','accepted_records','isolated_records','reason_counts']}, ensure_ascii=False))


if __name__ == '__main__':
    main()

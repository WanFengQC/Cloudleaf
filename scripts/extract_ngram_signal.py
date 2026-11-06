"""固定Google一元原件的只读研发投影；不推导词语转移或生产词频。"""
from pathlib import Path
from datetime import datetime, timezone
import argparse
import gzip
import hashlib
import json
import platform
import time

SOURCE_SHA = '6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86'
TOTALS_SHA = 'f4a136b63419c0f34dfbfcf76f501ac80a745ad535f5cc9bda2666189a35e9f3'
LEXICON_SHA = '03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
EXPANDED_SHA = 'f5535ee7d0c0a5ba18bff3b4019d150da0e63265b8e2abdebb2d442db45b95f0'
SOURCE_URL = 'https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/1-00000-of-00001.gz'
INDEX_URL = 'https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/chi_sim-1-ngrams_exports.html'
LICENSE_URL = 'https://creativecommons.org/licenses/by/3.0/'
LICENSE_SHA = 'e6bc9e9c474700b708f568bac9e5a8a9bcb2b1dad53442f5ba449fcb848b8e76'


def scan_observations(stream, wanted):
    observed = {}
    expanded = hashlib.sha256()
    seen = set()
    row_count = groups = 0
    for line in stream:
        expanded.update(line)
        fields = line.decode('utf-8').rstrip('\n').split('\t')
        token, *values = fields
        if token in seen or not token or not values:
            raise ValueError('固定源重复词头或缺三元组')
        seen.add(token)
        row_count += 1
        included = token in wanted
        count = window_groups = 0
        years = set()
        for value in values:
            triplet = value.split(',')
            if len(triplet) != 3 or any(not v.isascii() or not v.isdecimal() for v in triplet):
                raise ValueError('固定源三元组形状变化')
            year, raw_matches, _raw_volumes = map(int, triplet)
            if year in years:
                raise ValueError('词头逐年重复，不隐式合计重复记录')
            years.add(year)
            groups += 1
            if included and 2000 <= year <= 2019:
                count += raw_matches
                window_groups += 1
        if included and window_groups:
            observed[token] = {'raw_match_sum': count, 'year_groups': window_groups}
    return observed, expanded.hexdigest(), row_count, groups


def file_identity(path, expected):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    actual = digest.hexdigest()
    if actual != expected:
        raise ValueError(f'固定输入SHA不符：{path.name}')
    return {'path': str(path.resolve()), 'bytes': path.stat().st_size, 'sha256': actual}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--totalcounts', type=Path, required=True)
    parser.add_argument('--lexicon', type=Path, required=True)
    parser.add_argument('--license', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError('输出目录已存在，拒绝覆盖')
    started = time.perf_counter()
    inputs = {'source': file_identity(args.source, SOURCE_SHA),
              'totalcounts': file_identity(args.totalcounts, TOTALS_SHA),
              'lexicon': file_identity(args.lexicon, LEXICON_SHA)}
    license_bytes = args.license.read_bytes()
    if hashlib.sha256(license_bytes).hexdigest() != LICENSE_SHA:
        raise ValueError('需要固定CC BY3原文字节，不能替换许可或静默换行')
    text_ids = {}
    lines = args.lexicon.read_text(encoding='utf-8').splitlines()
    if (lines[0] != '# cloudleaf-lexicon-v2' or lines[1] !=
        'id\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id'):
        raise ValueError('运行时词典格式无效')
    for row in lines[2:]:
        fields = row.split('\t')
        if len(fields) != 9 or fields[3:5] != ['', 'unknown']:
            raise ValueError('固定词典结构或unknown语义变化')
        text_ids.setdefault(fields[2], []).append(fields[0])
    with gzip.open(args.source, 'rb') as stream:
        observed, expanded_sha, row_count, groups = scan_observations(stream, text_ids)
    if expanded_sha != EXPANDED_SHA or row_count != 508674 or groups != 19074163:
        raise ValueError('固定源完整扫描身份或分母变化')
    projected = sorted((entry_id, observation['raw_match_sum'])
                       for token, observation in observed.items() if observation['raw_match_sum'] > 0
                       for entry_id in text_ids[token])
    lines = [
        '# cloudleaf-language-v1',
        f'lexicon_sha256\t{LEXICON_SHA}',
        f'source_sha256\t{SOURCE_SHA}',
        'source_id\tgoogle-books-chi-sim-2019-fixed1gram',
        'license_id\tCC-BY-3.0',
        'tokenization\texact-headword:2000-2019:raw-match-inferred',
        'usage\tresearch_only',
        'kind\tlexical',
        'training_split\ttrain',
        'previous\tnext\tcount',
        *[f'<lexical>\t{entry_id}\t{count}' for entry_id, count in projected],
    ]
    payload = ('\n'.join(lines) + '\n').encode('utf-8')
    # 同正文的不同读音共享正文统计，不将其伪称逐读音频率。
    report = {'format': 'cloudleaf-round15-research-lexical-projection-v1',
        'usage': 'research_only', 'production_approved': False,
        'at_utc': datetime.now(timezone.utc).isoformat(),
        'inputs': inputs, 'source_url': SOURCE_URL, 'license_statement_url': INDEX_URL,
        'license_url': LICENSE_URL, 'copyright': '© 2019 Google',
        'license_sha256': hashlib.sha256(license_bytes).hexdigest(),
        'modification': 'Filter fixed 1gram raw records by exact unmodified CEDICT text and sum raw field2 in 2000-2019. Project identical text signal to stable reading IDs. No production frequency or transition statistics.',
        'source_status_unchanged': 'candidate-not-approved',
        'unit': 'inferred raw match count; not approved token-frequency, DF or Zipf',
        'totalcounts_used_as_denominator': False,
        'token_frequency': None, 'document_frequency': None, 'zipf': None,
        'missing_records_are_zero': False, 'runtime_frequency_modified': False,
        'transition_or_BOS_EOS_statistics_available': False,
        'window': [2000, 2019], 'source_rows': row_count, 'raw_triplet_fields': groups,
        'gzip_crc_verified': True, 'expanded_sha256': expanded_sha,
        'lexicon_entries': sum(map(len, text_ids.values())), 'lexicon_unique_texts': len(text_ids),
        'observed_texts': len(observed), 'projected_entry_ids': len(projected),
        'missing_entry_ids': sum(map(len, text_ids.values())) - len(projected),
        'observed_zero_values_excluded_from_model': sum(value['raw_match_sum'] == 0 for value in observed.values()),
        'signal_sha256': hashlib.sha256(payload).hexdigest(), 'signal_bytes': len(payload),
        'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'python': platform.python_version(), 'seconds': time.perf_counter() - started,
        'diagnostic_words_not_used_for_training_or_parameter_selection':
            {word: observed.get(word) for word in ['我', '握', '斡', '倭', '爱', '昵', '你', '很', '痕',
                                                  '好', '諕', '正在', '开发', '炌', '一', '法医',
                                                  '款', '中文', '输入法', '天气']},
        'remaining': ['v3 match semantics inferred', 'unknown original Chinese tokenizer version',
                      'truncation and POS alias protocol not accepted',
                      'historical books are not modern IME representativeness evidence',
                      'no independent gold', 'no word transition corpus']}
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / 'signal.tsv').write_bytes(payload)
    (args.output / 'LICENSE.txt').write_bytes(license_bytes)
    (args.output / 'NOTICE.txt').write_text(
        f'Google Books Ngram Viewer Chinese2019 compilation. © 2019 Google.\n{INDEX_URL}\n'
        f'Licensed CC BY 3.0: {LICENSE_URL}\n' + report['modification'] + '\n'
        'This is an explicit local research projection, not a production frequency package.\n',
        encoding='utf-8', newline='\n')
    (args.output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n',
                                             encoding='utf-8', newline='\n')
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()

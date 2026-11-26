"""只读固定Ngrams研究原件；原始字段诊断不产生生产词频或gold。"""
from __future__ import annotations

import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import re
import time

SOURCE_SHA256 = '6ab29862a8c904c261bb9d5a02a342285c4dbabf79cee5c4f8818076fd44fb86'
TOTALCOUNTS_SHA256 = 'f4a136b63419c0f34dfbfcf76f501ac80a745ad535f5cc9bda2666189a35e9f3'
BASE_URL = 'https://storage.googleapis.com/books/ngrams/books/20200217/chi_sim/'
TAGGED = re.compile(r'_(?:ADJ|ADP|ADV|CONJ|DET|NOUN|NUM|PRON|PRT|VERB|X|\.)$')


def analyze(source, totalcounts, *, expected_source_sha256=SOURCE_SHA256,
            expected_totalcounts_sha256=TOTALCOUNTS_SHA256):
    started = time.perf_counter()
    source, totalcounts = Path(source), Path(totalcounts)
    identities = {'source': file_identity(source), 'totalcounts': file_identity(totalcounts)}
    verified = (identities['source']['sha256'] == expected_source_sha256
                and identities['totalcounts']['sha256'] == expected_totalcounts_sha256)
    result = {'format': 'cloudleaf-ngram-raw-analysis-v1', 'schema_version': 1,
        'inputs': identities, 'expected_sha256': {'source': expected_source_sha256, 'totalcounts': expected_totalcounts_sha256},
        'integrity': {'sha256_verified': verified, 'gzip_crc_verified': False},
        'scan_completed': False, 'status': 'blocked-identity', 'fatal_error': None,
        'counts': {'rows': 0, 'triplet_fields': 0, 'valid_triplets': 0, 'totalcounts_records': 0, 'unique_tokens': 0, 'unique_token_utf8_bytes': 0},
        'anomaly_counts': {}, 'anomaly_examples': {}, 'example_limit_per_type': 5,
        'totalcounts_fields': {}, 'year_fields': {}, 'shape_rows': {},
        'structural_missing_token_year_pairs': None, 'totalcounts_years_without_export_groups': [],
        'production_approved': False, 'formal_frequency': {'token_frequency': None, 'document_frequency': None, 'zipf': None},
        'formal_language_metrics': {'top1': None, 'top5': None, 'mrr': None},
        'semantics': {'v3_field_meaning_status': 'inferred-not-production-contract',
            'observed_export_shape': 'token TAB integer,integer,integer ...',
            'proposed_triplet_interpretation': ['year', 'match_count', 'volume_count'],
            'totalcounts_proposed_interpretation': ['year', 'match_count', 'page_count', 'volume_count'],
            'basis': 'Fixed v3 observed shape and official v1/v2 examples; no explicit reviewed v3 contract.',
            'chinese_segmentation': 'Statistical system stated by official info; implementation/model/version/parameters unknown.',
            'v3_cutoff': None, 'v3_cutoff_reason': 'FAQ 40 books and v1/v2 >40 occurrences are not treated as the same v3 rule.',
            'missing_means_zero': False, 'field_3_sums_are_distinct_DF': False, 'all_field_2_sums_are_language_coverage': False,
            'tag_shape_is_POS_fact': False, 'duplicate_totals_policy': 'First record per year used for diagnostics; duplicates remain anomalies.'},
        'attribution': {'corpus': 'Google Books Chinese 2019, googlebooks-chi-sim-20200217',
            'source_url': BASE_URL + '1-00000-of-00001.gz', 'totalcounts_url': BASE_URL + 'totalcounts-1',
            'license_statement_url': BASE_URL + 'chi_sim-1-ngrams_exports.html', 'data_license': 'CC-BY-3.0',
            'license_url': 'https://creativecommons.org/licenses/by/3.0/', 'credit': 'Google Books Ngram compilation, Google',
            'modifications': 'Read-only structural diagnostics, field sums and limited record hashes; no source edits or approved frequency values.',
            'scope': 'Compilation only; not original book text rights. Explicit fixture hashes do not establish corpus authenticity.'},
        'limits': ['Global duplicate detection stores token strings, O(unique tokens) memory; year aggregation O(years).',
                   'UTF8 token byte sum is payload size, not allocated/live/peak process memory.',
                   'Field sums include aliases/tag forms; field3 sums do not deduplicate books across words or years.',
                   'Missing token-year pairs are structural and only computed for a complete anomaly-free scan; never observed zero.',
                   'Limited examples retain line/record SHA; full anomaly counters include later records and source originals remain available.']}
    if verified:
        try:
            totals = parse_totals(totalcounts.read_bytes(), result)
            scan_source(source, totals, result)
            result['scan_completed'] = True
            result['integrity']['gzip_crc_verified'] = True
            result['status'] = 'complete-with-anomalies' if result['anomaly_counts'] else 'complete'
        except (OSError, EOFError, UnicodeError) as error:
            result['status'] = 'partial'
            result['fatal_error'] = {'type': type(error).__name__, 'message': str(error)}
    result['anomaly_counts'] = dict(sorted(result['anomaly_counts'].items()))
    result['seconds'] = time.perf_counter() - started
    return result


def file_identity(path):
    digest = hashlib.sha256()
    size = 0
    with path.open('rb') as stream:
        while data := stream.read(1024 * 1024):
            digest.update(data)
            size += len(data)
    return {'path': str(path.resolve()), 'bytes': size, 'sha256': digest.hexdigest()}


def note(result, kind, location, raw, fields=None):
    counts = result['anomaly_counts']
    counts[kind] = counts.get(kind, 0) + 1
    examples = result['anomaly_examples'].setdefault(kind, [])
    if len(examples) < result['example_limit_per_type']:
        examples.append({**location, 'raw_record_sha256': hashlib.sha256(raw).hexdigest(), 'fields': fields})


def unsigned_fields(value, width):
    fields = value.split(',')
    if len(fields) != width:
        return None, 'malformed'
    if not all(part.isascii() and part.isdecimal() for part in fields):
        return None, 'invalid'
    return [int(part) for part in fields], None


def parse_totals(raw, result):
    totals = {}
    records = raw.decode('utf-8').split()
    result['counts']['totalcounts_records'] = len(records)
    for number, value in enumerate(records, 1):
        fields, error = unsigned_fields(value, 4)
        record = value.encode('utf-8')
        location = {'totalcounts_record': number}
        if error:
            note(result, error + '_totalcounts_' + ('record' if error == 'malformed' else 'number'), location, record)
        elif fields[0] in totals:
            note(result, 'duplicate_totalcounts_year', location, record, fields)
        else:
            totals[fields[0]] = fields[1:]
    result['totalcounts_fields'] = {str(year): totals[year] for year in sorted(totals)}
    result['totalcounts_year_range'] = [min(totals), max(totals)] if totals else None
    if not totals:
        note(result, 'no_valid_totalcounts', {}, raw)
    return totals


def token_shape(token):
    if TAGGED.search(token):
        return 'tagged_suffix_shape'
    if re.fullmatch(r'_[A-Z]+_', token):
        return 'uppercase_underscore_shape'
    return 'other_shape'


def scan_source(source, totals, result):
    tokens, years, shapes = set(), {}, Counter()
    digest, decompressed_bytes = hashlib.sha256(), 0
    try:
        with gzip.open(source, 'rb') as stream:
            for number, raw in enumerate(stream, 1):
                digest.update(raw)
                decompressed_bytes += len(raw)
                result['counts']['rows'] += 1
                values = raw.decode('utf-8').rstrip('\r\n').split('\t')
                token = values[0]
                location = {'line': number}
                if not token or len(values) < 2:
                    note(result, 'malformed_row', location, raw)
                    continue
                result['counts']['triplet_fields'] += len(values) - 1
                if token in tokens:
                    note(result, 'duplicate_token', location, raw)
                else:
                    tokens.add(token)
                    result['counts']['unique_token_utf8_bytes'] += len(token.encode('utf-8'))
                shape = token_shape(token)
                shapes[shape] += 1
                seen_years, previous = set(), None
                for index, value in enumerate(values[1:], 1):
                    fields, error = unsigned_fields(value, 3)
                    location = {'line': number, 'triplet': index}
                    if error:
                        note(result, error + '_triplet' + ('_number' if error == 'invalid' else ''), location, raw)
                        continue
                    year, field_2, field_3 = fields
                    result['counts']['valid_triplets'] += 1
                    if year in seen_years:
                        note(result, 'duplicate_year', location, raw, fields)
                    if previous is not None and year < previous:
                        note(result, 'nonmonotonic_year', location, raw, fields)
                    seen_years.add(year)
                    previous = year
                    if field_3 > field_2:
                        note(result, 'field_3_exceeds_field_2', location, raw, fields)
                    if year not in totals:
                        note(result, 'year_missing_from_totalcounts', location, raw, fields)
                    else:
                        if field_2 > totals[year][0]:
                            note(result, 'field_2_exceeds_total', location, raw, fields)
                        if field_3 > totals[year][2]:
                            note(result, 'field_3_exceeds_total', location, raw, fields)
                    aggregate = years.setdefault(year, {'groups': 0, 'field_2_sum': 0, 'field_3_sum': 0, 'shapes': {}})
                    aggregate['groups'] += 1
                    aggregate['field_2_sum'] += field_2
                    aggregate['field_3_sum'] += field_3
                    group = aggregate['shapes'].setdefault(shape, {'groups': 0, 'field_2_sum': 0, 'field_3_sum': 0})
                    group['groups'] += 1
                    group['field_2_sum'] += field_2
                    group['field_3_sum'] += field_3
    finally:
        result['counts']['unique_tokens'] = len(tokens)
        result['uncompressed_bytes'] = decompressed_bytes
        result['uncompressed_sha256'] = digest.hexdigest()
        result['year_fields'] = {str(year): years[year] for year in sorted(years)}
        result['export_year_range'] = [min(years), max(years)] if years else None
        result['shape_rows'] = dict(sorted(shapes.items()))
        result['totalcounts_years_without_export_groups'] = sorted(totals.keys() - years.keys())
    if not result['anomaly_counts']:
        result['structural_missing_token_year_pairs'] = len(tokens) * len(totals) - result['counts']['valid_triplets']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--totalcounts', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('输出已存在，禁止覆盖')
    try:
        result = analyze(args.source, args.totalcounts)
        result['tool_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open('x', encoding='utf-8', newline='\n') as stream:
            json.dump(result, stream, ensure_ascii=False, indent=2)
            stream.write('\n')
    except OSError as error:
        parser.error(str(error))
    print(json.dumps({'status': result['status'], 'rows': result['counts']['rows'],
                      'anomaly_counts': result['anomaly_counts'], 'production_approved': False}, ensure_ascii=False))
    return 0 if result['status'] == 'complete' else 2


if __name__ == '__main__':
    raise SystemExit(main())

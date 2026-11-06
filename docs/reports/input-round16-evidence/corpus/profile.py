from pathlib import Path
from collections import Counter, defaultdict
import datetime
import hashlib
import json
import re
import unicodedata
from html.parser import HTMLParser

ROOT = Path(__file__).resolve().parent

class Text(HTMLParser):
    def __init__(self):
        super().__init__()
        self.parts = []
    def handle_data(self, data):
        self.parts.append(data)

for name in ['terms', 'downloads', 'using-corpus', 'faq', 'CC-BY-2.0-FR-legalcode']:
    p = Text()
    p.feed((ROOT / f'{name}.html').read_text(encoding='utf-8'))
    (ROOT / f'{name}.txt').write_text('\n'.join(t.strip() for t in p.parts if t.strip()), encoding='utf-8')

rows = []
shapes = Counter()
bad = []
ids = set()
authors = Counter()
dates = []
modified = []
for n, line in enumerate((ROOT / 'cmn_sentences_detailed.tsv').read_text(encoding='utf-8').splitlines(), 1):
    f = line.split('\t')
    shapes[len(f)] += 1
    if len(f) != 6 or not f[0].isdigit() or f[1] != 'cmn' or f[0] in ids:
        bad.append({'line': n, 'fields': len(f), 'id': f[0]})
        continue
    ids.add(f[0])
    rows.append(f)
    authors[f[3]] += 1
    if f[4] != '\\N': dates.append(f[4])
    if f[5] != '\\N': modified.append(f[5])

def norm(s):
    return ''.join(c for c in unicodedata.normalize('NFKC', s) if unicodedata.category(c)[0] not in 'PZS').lower()

normalized = defaultdict(list)
for r in rows:
    normalized[norm(r[2])].append(r[0])

targets = ['我爱你', '这是一个比较复杂的问题', '今天天气很好', '我正在开发一款中文输入法']
target_matches = {}
for target in targets:
    target_matches[target] = {'exact_normalized': [r[0] for r in rows if norm(r[2]) == target],
                               'contains_normalized': [r[0] for r in rows if target in norm(r[2])],
                               'raw_exact': [r[0] for r in rows if r[2] == target]}

tags = Counter()
tagged_ids = set()
for line in (ROOT / 'cmn_tags.tsv').read_text(encoding='utf-8').splitlines():
    f = line.split('\t')
    if len(f) == 2:
        tagged_ids.add(f[0])
        tags[f[1]] += 1

base_types = Counter()
for line in (ROOT / 'cmn_sentences_base.tsv').read_text(encoding='utf-8').splitlines():
    i, base = line.split('\t')
    base_types['original' if base == '0' else 'unknown' if base == '\\N' else 'translation'] += 1

report = {
    'source': 'Tatoeba official Mandarin Chinese weekly export, Last-Modified 2026-10-31',
    'global_source_family': 'tatoeba-community',
    'usage': 'research_only',
    'formal_gold': False,
    'encoding': 'strict UTF-8',
    'rows': len(rows), 'field_shapes': dict(shapes), 'bad_records': bad,
    'unique_sentence_ids': len(ids), 'distinct_authors': len(authors),
    'orphan_author_rows': authors['\\N'], 'empty_author_rows': authors[''],
    'date_added_min': min(dates), 'date_added_max': max(dates),
    'last_modified_min': min(modified), 'last_modified_max': max(modified),
    'max_characters': max(len(r[2]) for r in rows),
    'total_characters': sum(len(r[2]) for r in rows),
    'text_sha256': hashlib.sha256('\n'.join(r[2] for r in rows).encode('utf-8')).hexdigest(),
    'exact_distinct_texts': len({r[2] for r in rows}),
    'nfkc_lower_punctuation_space_symbol_stripped_distinct_texts': len(normalized),
    'duplicate_normalized_groups': sum(len(v) > 1 for v in normalized.values()),
    'duplicate_normalized_extra_rows': sum(len(v)-1 for v in normalized.values()),
    'historical_target_matches': target_matches,
    'original_translation_counts': dict(base_types),
    'tagged_sentence_count': len(tagged_ids), 'tags': dict(tags),
    'privacy_shape_flags': {
        'email_like_rows': sum(bool(re.search(r'[^\s@]+@[^\s@]+\.[^\s@]+', r[2])) for r in rows),
        'url_like_rows': sum(bool(re.search(r'https?://|www\.', r[2])) for r in rows),
        'phone_like_7plus_consecutive_ascii_digits_rows': sum(bool(re.search(r'[0-9]{7,}',r[2])) for r in rows),
        'note': 'shape flags only, zero flags do not prove absence of personal information; do not publish full raw sentences or contributors as product data without review',
    },
    'source_warnings': ['crowdsourced learner examples and translations, not a corpus of naturally sampled IME behavior',
                        'Mandarin language code does not imply only simplified Chinese; no script conversion done',
                        'current export is mutable upstream; frozen local SHA is version identity, not a permanently versioned server URL',
                        'base and same-language links are not a complete all-language translation graph',
                        'same global source family cannot constitute independent gold or independent-language-quality evidence',
                        'no annotation/reviewer identities inferred from existing contributor usernames'],
}
(ROOT / 'profile.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(report, ensure_ascii=False, indent=2))

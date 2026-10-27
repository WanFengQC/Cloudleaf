"""独立离线语言侧包的类型/引用门禁；不推断频率、人工语义或真实授权。"""
from __future__ import annotations

import argparse
from datetime import date
import json
import math
from pathlib import Path
import re

SHA = re.compile('[a-f0-9]{64}\\Z')


def require(value, message):
    if not value:
        raise ValueError(message)


def shape(value, fields):
    require(isinstance(value, dict) and set(value) == set(fields.split()), '侧包字段集合错误')


def text(value):
    return isinstance(value, str) and 0 < len(value) <= 4096 and value == value.strip() and not any(ord(c) < 32 for c in value)


def digest(value):
    return isinstance(value, str) and bool(SHA.fullmatch(value))


def number(value):
    return type(value) in {int, float} and math.isfinite(value)


def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def validate_sidecar(package: dict, known_entry_ids: set[str], *, expected_lexicon_sha256: str) -> dict:
    try:
        return _validate(package, known_entry_ids, expected_lexicon_sha256)
    except (KeyError, TypeError, OverflowError) as error:
        raise ValueError('侧包结构或值类型错误') from error


def _validate(package, known, expected):
    shape(package, 'format schema_version synthetic lexicon_sha256 sources entries observations senses translations phrases')
    require(package['format'] == 'cloudleaf-linguistic-sidecar-v1' and type(package['schema_version']) is int
            and package['schema_version'] == 1 and type(package['synthetic']) is bool, '侧包版本错误')
    require(digest(expected) and package['lexicon_sha256'] == expected, '侧包词典身份不匹配')
    for name in ['sources', 'entries', 'observations', 'senses', 'translations', 'phrases']:
        require(isinstance(package[name], list) and len(package[name]) <= 1000000, '侧包集合类型/数量错误')
    sources = {}
    for source in package['sources']:
        shape(source, 'id version sha256 license_id license_sha256 audit_sha256 review_status scope')
        require(all(text(source[k]) for k in ['id', 'version', 'license_id'])
                and all(digest(source[k]) for k in ['sha256','license_sha256','audit_sha256'])
                and source['review_status'] == 'approved' and source['id'] not in sources, '来源批准或身份错误')
        require(isinstance(source['scope'], list) and source['scope']
                and len(source['scope']) == len(set(source['scope']))
                and set(source['scope']) <= {'frequency', 'linguistic-annotation'}, '来源范围错误')
        sources[source['id']] = source

    def source_scope(identity, scope):
        require(identity in sources and scope in sources[identity]['scope'], '来源缺失或未批准本范围')

    def review(value):
        shape(value, 'reviewers disputed adjudicator')
        reviewers = value['reviewers']
        require(isinstance(reviewers, list) and len(reviewers) == 2 and all(text(r) for r in reviewers)
                and len(set(reviewers)) == 2 and type(value['disputed']) is bool, '需要两个不同reviewer')
        adjudicator = value['adjudicator']
        require(adjudicator is None or (text(adjudicator) and adjudicator not in reviewers), '仲裁者身份错误')
        require(not value['disputed'] or adjudicator is not None, '有争议记录缺独立仲裁者')

    def provenance(value):
        source_scope(value['source_id'], 'linguistic-annotation')
        shape(value['evidence'], 'unit_id sha256')
        require(text(value['evidence']['unit_id']) and digest(value['evidence']['sha256']), '注释缺来源证据')
        review(value['review'])

    def pos(value):
        if value is None:
            return
        shape(value, 'tag tagset tagset_version source_id evidence review')
        require(all(text(value[k]) for k in ['tag','tagset','tagset_version']), 'POS缺tagset版本')
        provenance(value)

    entry_ids = set()
    for entry in package['entries']:
        shape(entry, 'entry_id pos')
        require(entry['entry_id'] in known and entry['entry_id'] not in entry_ids, '侧包entry重复或不存在')
        entry_ids.add(entry['entry_id'])
        pos(entry['pos'])

    def corpus(value):
        shape(value, 'id version sha256 period tokenizer')
        require(text(value['id']) and text(value['version']) and digest(value['sha256']), '语料身份错误')
        period = value['period']
        require(isinstance(period, list) and len(period) == 2 and all(text(p) for p in period), '语料时期缺失')
        require(date.fromisoformat(period[0]) <= date.fromisoformat(period[1]), '语料时期顺序错误')
        tokenizer = value['tokenizer']
        shape(tokenizer, 'id version config_sha256')
        require(text(tokenizer['id']) and text(tokenizer['version']) and digest(tokenizer['config_sha256']), '分词/预处理身份错误')

    observations = {}
    unknown = zero = 0
    for row in package['observations']:
        shape(row, 'id entry_id source_id metric coverage value numerator denominator denominator_unit transform corpus')
        require(text(row['id']) and row['id'] not in observations and row['entry_id'] in known, '观测ID重复或词条不存在')
        source_scope(row['source_id'], 'frequency')
        corpus(row['corpus'])
        require(row['metric'] in {'token_frequency','document_frequency','zipf'}
                and row['coverage'] in {'observed','unobserved','below_cutoff'}, '频率类型或coverage错误')
        missing = row['coverage'] != 'observed'
        if missing:
            require(row['value'] is None and row['numerator'] is None and row['denominator'] is None, 'unknown不能补0或伪观测')
            unknown += 1
        else:
            require(number(row['value']), '已观测值必须有限且拒绝bool')
            zero += row['value'] == 0
        if row['metric'] in {'token_frequency','document_frequency'}:
            unit = 'tokens' if row['metric'] == 'token_frequency' else 'documents'
            require(row['denominator_unit'] == unit and row['transform'] is None, 'token/DF单位或变换错误')
            if not missing:
                require(integer(row['numerator']) and integer(row['denominator'], 1)
                        and row['numerator'] <= row['denominator'], '观测分母/计数错误')
                require((row['value'] == 0) == (row['numerator'] == 0), '正计数下溢不能变为观测零')
                expected_value = row['numerator'] / row['denominator'] if unit == 'tokens' else row['numerator']
                require(unit != 'documents' or integer(row['value']), 'DF原始文档数必须为整数')
                require(math.isclose(row['value'], expected_value, rel_tol=1e-12, abs_tol=0), '频率值与口径不符')
        else:
            require(row['denominator_unit'] == 'per_billion_tokens' and row['numerator'] is None
                    and row['denominator'] is None, 'Zipf基准/计数错误')
            transform = row['transform']
            require(isinstance(transform, dict) and transform.get('kind') in {'native','derived-token'}, 'Zipf需原生定义或token母观测')
            require(type(transform.get('log_base')) is int and transform['log_base'] == 10
                    and type(transform.get('scale')) is int and transform['scale'] == 1000000000, 'Zipf公式基准错误')
            if transform['kind'] == 'native':
                shape(transform, 'kind definition_sha256 log_base scale')
                require(digest(transform['definition_sha256']), '原生Zipf缺定义摘要')
            else:
                shape(transform, 'kind parent_id log_base scale')
                require(text(transform['parent_id']), 'Zipf母观测ID错误')
            if not missing:
                require(row['value'] <= 9, 'Zipf值超过token比例可能范围')
        observations[row['id']] = row
    for row in observations.values():
        if row['metric'] != 'zipf' or row['transform']['kind'] != 'derived-token':
            continue
        parent = observations.get(row['transform']['parent_id'])
        require(parent is not None and parent['metric'] == 'token_frequency' and parent['coverage'] == 'observed'
                and parent['value'] > 0 and row['coverage'] == 'observed', 'Zipf只能由已观测正token比例派生')
        require(all(row[key] == parent[key] for key in ['entry_id','source_id','corpus']), 'Zipf不能换词、换来源或换语料/预处理身份')
        require(math.isclose(row['value'], math.log10(parent['value']) + 9, rel_tol=1e-12, abs_tol=1e-12), 'Zipf值与公式不符')

    senses = {}
    for value in package['senses']:
        shape(value, 'id entry_id language definition pos source_id evidence review')
        require(text(value['id']) and value['id'] not in senses and value['entry_id'] in known
                and text(value['language']) and text(value['definition']), '义项身份/定义错误')
        provenance(value)
        pos(value['pos'])
        senses[value['id']] = value
    seen = set()
    for value in package['translations']:
        shape(value, 'id source_sense_id target_sense_id relation source_id evidence review')
        require(text(value['id']) and value['id'] not in seen and value['source_sense_id'] in senses
                and value['target_sense_id'] in senses and value['source_sense_id'] != value['target_sense_id'], '翻译存在孤儿或重复关系')
        require(value['relation'] in {'equivalent','broader','narrower','related'}, '翻译关系错误')
        seen.add(value['id'])
        provenance(value)
    phrase_ids = set()
    for value in package['phrases']:
        shape(value, 'id entry_id kind components source_id evidence review')
        require(text(value['id']) and value['id'] not in phrase_ids and value['entry_id'] in known
                and value['kind'] in {'idiom','collocation','multiword'}, '短语身份/类型错误')
        require(isinstance(value['components'], list) and 1 <= len(value['components']) <= 64
                and all(component in known for component in value['components']), '短语组件引用错误')
        phrase_ids.add(value['id'])
        provenance(value)
    return {'schema_version':1, 'verified':True, 'synthetic':package['synthetic'],
            'lexicon_sha256':expected, 'observations':len(observations), 'unknown_observations':unknown,
            'observed_zero':zero, 'annotated_senses':len(senses), 'translation_links':len(seen),
            'phrases':len(phrase_ids), 'language_accuracy_valid':False,
            'scope':'Structure, supplied provenance/review and mathematical consistency only; no automatic proof of rights, reviewers or linguistic truth'}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sidecar',type=Path,required=True)
    parser.add_argument('--facts',type=Path,required=True)
    parser.add_argument('--facts-sha256',required=True)
    args=parser.parse_args()
    import hashlib
    facts=args.facts.read_bytes()
    require(hashlib.sha256(facts).hexdigest()==args.facts_sha256,'事实文件SHA不匹配')
    require(args.sidecar.stat().st_size <= 64*1024*1024,'侧包过大')
    known={json.loads(line)['id'] for line in facts.decode('utf-8').splitlines()}
    result=validate_sidecar(json.loads(args.sidecar.read_text(encoding='utf-8')),known,
                            expected_lexicon_sha256=args.facts_sha256)
    print(json.dumps(result,ensure_ascii=False))


if __name__=='__main__':main()

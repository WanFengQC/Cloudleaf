"""离线人工IME任务、双审、泄漏检查和冻结；不生成答案或证明真人身份。"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import unicodedata

SHA = re.compile(r'[0-9a-f]{64}\Z')
FIELDS = ('id', 'source_id', 'source_unit_id', 'family_id', 'near_duplicate_group', 'split',
          'source_binding_sha256', 'author_id', 'input_intent', 'pinyin', 'acceptable_outputs', 'key_trace', 'scenario')
SCENARIOS = ('日常短句', '长句', '显式分音', '中间编辑', '退格修改', '取消',
             '未登录词', '专名', '数字混输', '标点', '领域术语', '上下文歧义')


def canonical(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')) + '\n').encode('utf-8')


def source_digest(source):
    return hashlib.sha256(canonical(source)).hexdigest()


def annotation_digest(task):
    return hashlib.sha256(canonical({key: task.get(key) for key in FIELDS})).hexdigest()


def review_binding_digest(packet):
    return hashlib.sha256(canonical({key: packet[key] for key in
                                   ['protocol_sha256', 'preprocessing_sha256', 'lineage_audit_sha256']})).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def pending_packet():
    return {'schema_version': 1, 'format': 'cloudleaf-ime-annotation-v1', 'synthetic': False,
            'dictionary_derived': False, 'protocol_sha256': None, 'preprocessing_sha256': None,
            'lineage_audit_sha256': None, 'custodian_id': None, 'sources': [],
            'tasks': [{'id': f'ime-r6-{index:02}', 'status': 'pending', 'scenario': scenario,
                       **{key: None for key in FIELDS if key not in {'id', 'scenario', 'acceptable_outputs', 'key_trace'}},
                       'acceptable_outputs': [], 'key_trace': [], 'reviews': [], 'adjudication': None}
                      for index, scenario in enumerate(SCENARIOS, 1)]}


def validate(packet):
    require(packet.get('schema_version') == 1 and packet.get('format') == 'cloudleaf-ime-annotation-v1', '任务协议版本不符')
    require(type(packet.get('synthetic')) is bool and packet.get('dictionary_derived') is False, '不得用词典派生答案替代独立人工任务')
    require(all(isinstance(packet.get(key), str) and SHA.fullmatch(packet[key]) for key in
                ['protocol_sha256', 'preprocessing_sha256', 'lineage_audit_sha256']), '协议/预处理/来源家族审计SHA未交付')
    binding = review_binding_digest(packet)
    require(isinstance(packet.get('custodian_id'), str) and packet['custodian_id'].strip(), '缺独立保管者')
    sources, artifacts = {}, {}
    for source in packet['sources']:
        key = source['id']
        require(isinstance(key, str) and key and key not in sources, '来源ID为空或重复')
        require(all(isinstance(source.get(field), str) and source[field].strip() for field in
                    ['version', 'author', 'lineage_id']), '来源作者/版本/家族缺失')
        require(isinstance(source.get('artifact_sha256'), str) and SHA.fullmatch(source['artifact_sha256']), '来源原件SHA无效')
        grant = source['authorization']
        require(grant.get('status') == 'approved' and isinstance(grant.get('sha256'), str)
                and SHA.fullmatch(grant['sha256']) and {'evaluation', 'modification', 'redistribution'} <= set(grant.get('scope', [])), '来源授权未明确批准所需范围')
        require(artifacts.setdefault(source['artifact_sha256'], source['lineage_id']) == source['lineage_id'], '同原件换来源ID但不同lineage')
        sources[key] = source
    tasks, ids, owners, people = packet['tasks'], set(), {}, set()
    require(isinstance(tasks, list) and bool(tasks), '没有实际任务')
    for task in tasks:
        require(task.get('status') == 'completed', '存在未完成任务，不能冻结')
        require(all(isinstance(task.get(key), str) and task[key].strip() for key in FIELDS
                    if key not in {'acceptable_outputs', 'key_trace'}), '任务意图/拼音/场景/身份字段缺失')
        require(task['id'] not in ids and task['source_id'] in sources and task['split'] in {'train', 'dev', 'heldout'}, '任务重复/来源缺失/split无效')
        ids.add(task['id'])
        require(task['source_binding_sha256'] == source_digest(sources[task['source_id']]), '任务审核绑定的具体来源/授权已变化')
        for key in ['acceptable_outputs', 'key_trace']:
            require(isinstance(task[key], list) and bool(task[key]) and all(isinstance(item, str) and item.strip() for item in task[key]), '可接受输出或完整按键轨迹未交付')
        reviews = task['reviews']
        require(isinstance(reviews, list) and len(reviews) == 2, '必须恰有两位独立审核员')
        reviewer_ids = [row['reviewer_id'] for row in reviews]
        require(all(isinstance(name, str) and name.strip() for name in reviewer_ids)
                and len(set(reviewer_ids)) == 2 and task['author_id'] not in reviewer_ids, '作者和两位审核员必须不同')
        bound = annotation_digest(task)
        require(all(row.get('annotation_sha256') == bound and row.get('protocol_sha256') == packet['protocol_sha256']
                    and row.get('review_binding_sha256') == binding
                    and row.get('decision') in {'approve', 'reject'} for row in reviews), '审核未绑定当前注释/协议或决议无效')
        people.update([task['author_id'], *reviewer_ids])
        if any(row['decision'] != 'approve' for row in reviews):
            decision = task.get('adjudication') or {}
            arbiter = decision.get('adjudicator_id')
            require(isinstance(arbiter, str) and arbiter.strip() and arbiter not in {task['author_id'], *reviewer_ids}
                    and decision.get('decision') == 'approve' and decision.get('annotation_sha256') == bound
                    and decision.get('protocol_sha256') == packet['protocol_sha256']
                    and decision.get('review_binding_sha256') == binding, '分歧/拒绝须独立仲裁当前注释及全部处理协议')
            people.add(arbiter)
        source = sources[task['source_id']]
        normalized = task['pinyin'].lower().replace('ü', 'v')
        rough_intent = ''.join(character for character in unicodedata.normalize('NFKC', task['input_intent']).casefold() if character.isalnum())
        keys = [('family', task['family_id']), ('near', task['near_duplicate_group']),
                ('unit', source['lineage_id'], task['source_unit_id']), ('input', normalized), ('intent', rough_intent)]
        for key in keys:
            require(owners.setdefault(key, task['split']) == task['split'], '全局家族/近重复/lineage单元/规范输入或意图跨split')
    require(packet['custodian_id'] not in people, '保管者不得兼任作者/审核员/仲裁者')
    require({task['split'] for task in tasks} == {'train', 'dev', 'heldout'}, '三个split须由保管者明确分配')
    return {'tasks': len(tasks), 'sources': len(sources), 'splits': dict(Counter(task['split'] for task in tasks))}


def audit(packet):
    try:
        details = validate(packet)
        issues = []
    except (ValueError, KeyError, TypeError) as error:
        details, issues = None, [str(error)]
    return {'structurally_ready': not issues, 'details': details, 'issues': issues,
            'formal_language_ready': False, 'human_identity_or_permission_independently_proven': False,
            'top1': None, 'top5': None, 'mrr': None,
            'limitations': ['只核对声明；不证明真人/授权/语义近重复，也不满足B正式配额、封存和参数门禁。']}


def freeze(packet, output):
    checked = audit(packet)
    require(checked['structurally_ready'], '; '.join(checked['issues']))
    require(not output.exists(), '输出已存在，禁止覆盖冻结材料')
    output.mkdir(parents=True)
    index = []
    for task in packet['tasks']:
        index.append({'case_id': task['id'], 'source_id': task['source_id'], 'source_unit_id': task['source_unit_id'],
                      'family_id': task['family_id'], 'near_duplicate_group': task['near_duplicate_group'],
                      'split': task['split'], 'normalized_input_sha256': hashlib.sha256(task['pinyin'].lower().replace('ü', 'v').encode()).hexdigest(),
                      'task_sha256': annotation_digest(task)})
    (output / 'family-index.jsonl').write_bytes(b''.join(canonical(row) for row in index))
    for split in ['train', 'dev', 'heldout']:
        (output / (split + '-tasks.jsonl')).write_bytes(b''.join(canonical(task) for task in packet['tasks'] if task['split'] == split))
    # 公共元信息只列来源/协议和ID，不能再复制一份heldout答案。
    metadata = {key: value for key, value in packet.items() if key != 'tasks'}
    metadata['task_ids'] = [task['id'] for task in packet['tasks']]
    (output / 'packet-metadata.json').write_bytes(canonical(metadata))
    manifest = {**checked, 'format': 'cloudleaf-ime-freeze-v1', 'synthetic': packet['synthetic'],
                'heldout_encrypted': False, 'custodian_id': packet['custodian_id'],
                'protocol_sha256': packet['protocol_sha256'], 'preprocessing_sha256': packet['preprocessing_sha256'],
                'lineage_audit_sha256': packet['lineage_audit_sha256'],
                'review_binding_schema_version': 1, 'review_binding_sha256': review_binding_digest(packet),
                'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                'files_sha256': {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(output.iterdir())},
                'B_handoff': '仅family-index字段显式兼容；来源许可原件/审核/逐源protocol和review、2000heldout家族及70/15/15、独立加密封存和参数冻结仍须旧B门禁核验。'}
    (output / 'manifest.json').write_bytes(canonical(manifest))
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packet', type=Path)
    parser.add_argument('--pending-output', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--freeze', action='store_true')
    args = parser.parse_args()
    if args.pending_output:
        require(not args.packet and not args.output and not args.freeze and not args.pending_output.exists(), '待填队列输出已存在或参数冲突')
        args.pending_output.parent.mkdir(parents=True, exist_ok=True)
        args.pending_output.write_bytes(canonical(pending_packet()))
        return
    require(args.packet is not None and args.output is not None, '需要packet和新output')
    value = json.loads(args.packet.read_text(encoding='utf-8'))
    if args.freeze:
        result = freeze(value, args.output)
    else:
        require(not args.output.exists(), '审计输出已存在')
        result = audit(value)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(canonical(result))
    print(json.dumps(result, ensure_ascii=False))
    if not result['structurally_ready']:
        raise SystemExit(2)


if __name__ == '__main__':
    main()

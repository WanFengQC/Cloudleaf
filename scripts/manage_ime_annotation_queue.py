"""导出单人私有作者草稿及缺件进度；不生成正式gold或人工决定。"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path

import check_ime_task_handoff as handoff

base = handoff.base
PACKET_FIELDS = {'schema_version', 'format', 'synthetic', 'dictionary_derived', 'custodian_id',
                 'sources', 'tasks', *handoff.BINDINGS}
TASK_FIELDS = {*handoff.FIELDS, 'status', 'reviews', 'adjudication'}
QUEUE_FIELDS = {'schema_version', 'experiment', 'usage', 'origin', 'authorization_files', 'packet'}


def validate_packet(packet):
    base.require(isinstance(packet, dict) and set(packet) == PACKET_FIELDS
                 and packet['schema_version'] == 1 and packet['format'] == 'cloudleaf-ime-annotation-v1', '作者源packet字段或版本不符')
    base.require(packet['synthetic'] is False and packet['dictionary_derived'] is False, '作者队列不能接收合成或词典派生材料')
    base.require(isinstance(packet['sources'], list) and isinstance(packet['tasks'], list), '来源和任务必须为列表')
    source_ids = set()
    for source in packet['sources']:
        base.require(isinstance(source, dict) and isinstance(source.get('id'), str) and source['id'].strip()
                     and source['id'] not in source_ids, '来源ID缺失或重复')
        grant = source.get('authorization')
        base.require(grant is None or (isinstance(grant, dict) and isinstance(grant.get('scope', []), list)
                     and all(isinstance(scope, str) for scope in grant.get('scope', []))), '授权范围声明格式错误')
        source_ids.add(source['id'])
    task_ids = set()
    for task in packet['tasks']:
        base.require(isinstance(task, dict) and set(task) == TASK_FIELDS and isinstance(task['id'], str)
                     and task['id'].strip() and task['id'] not in task_ids, '任务字段或ID不符/重复')
        base.require(task['split'] is None, '只接受未划分的作者任务；禁止输入冻结或heldout包')
        base.require(task['status'] in {'pending', 'completed'} and isinstance(task['reviews'], list)
                     and all(isinstance(review, dict) for review in task['reviews'])
                     and (task['adjudication'] is None or isinstance(task['adjudication'], dict)), '任务状态或审核声明格式错误')
        base.require(all(task[field] is None or isinstance(task[field], str) for field in handoff.FIELDS
                         if field not in {'acceptable_outputs', 'key_trace', 'split'})
                     and all(isinstance(task[field], list) and all(isinstance(text, str) for text in task[field])
                             for field in ['acceptable_outputs', 'key_trace']), '作者输入字段类型错误')
        task_ids.add(task['id'])
    return task_ids, source_ids


def export_queue(path, expected_sha256):
    source = base.checked({'path': str(path.resolve()), 'sha256': expected_sha256}, Path.cwd())
    packet = base.read_json(source)
    task_ids, _ = validate_packet(packet)
    base.require(task_ids, '作者任务为空')
    return {'schema_version': 1, 'experiment': 'cloudleaf-authoring-queue-v1', 'usage': 'internal-development-only',
            'origin': {'packet_sha256': expected_sha256, 'task_ids': sorted(task_ids)},
            'authorization_files': {}, 'packet': packet}


def authorization_progress(sources, documents, folder):
    result = {}
    for source in sources:
        issues = []
        grant = source.get('authorization') or {}
        if grant.get('status') != 'approved' or not {'evaluation', 'modification', 'redistribution'} <= set(grant.get('scope', [])):
            issues.append('缺所需评估/修改/再分发授权范围声明')
        descriptor = documents.get(source['id'])
        actual_sha = None
        try:
            path = base.checked(descriptor, folder)
            actual_sha = base.digest(path)
            base.require(grant.get('sha256') == actual_sha, '文件与来源授权声明SHA不一致')
        except (ValueError, KeyError, TypeError, OSError) as error:
            issues.append(str(error))
        result[source['id']] = {'file_and_scope_consistent': not issues, 'actual_file_sha256': actual_sha, 'issues': issues,
                                'signed_authorization_or_rights_proven': False}
    return result


def personal_complete(task):
    strings = [key for key in handoff.FIELDS if key not in {'split', 'acceptable_outputs', 'key_trace'}]
    return (all(isinstance(task[key], str) and task[key].strip() for key in strings)
            and all(task[key] and all(text.strip() for text in task[key]) for key in ['acceptable_outputs', 'key_trace']))


def review_progress(task, packet, source_matches, protocols_present):
    issues = []
    task_digest = handoff.annotation_digest(task)
    binding = handoff.review_binding_digest(packet)
    if not protocols_present:
        issues.append('三协议SHA尚未全部填写，不能计为当前审核')
    if not source_matches:
        issues.append('任务来源缺失或来源绑定已变更')
    reviews = task['reviews']
    reviewers = [review.get('reviewer_id') for review in reviews]
    independent = (len(reviewers) == 2 and all(isinstance(person, str) and person.strip() for person in reviewers)
                   and len(set(reviewers)) == 2 and task['author_id'] not in reviewers)
    if not independent:
        issues.append('缺两名不同且非作者的审核ID声明')
    current = all(review.get('annotation_sha256') == task_digest
                  and review.get('protocol_sha256') == packet['protocol_sha256']
                  and review.get('review_binding_sha256') == binding
                  and review.get('decision') in {'approve', 'reject'} for review in reviews)
    if not current:
        issues.append('审核声明未绑定当前任务和三协议')
    arbitration = 'not-required'
    if any(review.get('decision') == 'reject' for review in reviews):
        declaration = task['adjudication'] or {}
        person = declaration.get('adjudicator_id')
        resolved = (isinstance(person, str) and person.strip() and person not in [task['author_id'], *reviewers]
                    and declaration.get('decision') == 'approve' and declaration.get('annotation_sha256') == task_digest
                    and declaration.get('protocol_sha256') == packet['protocol_sha256']
                    and declaration.get('review_binding_sha256') == binding)
        arbitration = 'current-independent-declaration' if resolved else 'missing-or-stale'
        if not resolved:
            issues.append('分歧缺独立且绑定当前内容的仲裁声明')
    return {'current_declarations_consistent': not issues and personal_complete(task), 'issues': issues,
            'task_sha256': task_digest, 'review_binding_sha256': binding, 'arbitration': arbitration,
            'human_independence_proven': False}


def status(path):
    value = base.read_json(path)
    base.require(isinstance(value, dict) and set(value) == QUEUE_FIELDS and value['schema_version'] == 1
                 and value['experiment'] == 'cloudleaf-authoring-queue-v1' and value['usage'] == 'internal-development-only', '作者队列版本或用途不符')
    task_ids, source_ids = validate_packet(value['packet'])
    origin = value['origin']
    base.require(isinstance(origin, dict) and set(origin) == {'packet_sha256', 'task_ids'}
                 and isinstance(origin['packet_sha256'], str) and base.SHA.fullmatch(origin['packet_sha256'])
                 and isinstance(origin['task_ids'], list) and all(isinstance(key, str) and key for key in origin['task_ids'])
                 and len(set(origin['task_ids'])) == len(origin['task_ids']) and set(origin['task_ids']) <= task_ids, '原始任务槽丢失或来源SHA无效')
    documents = value['authorization_files']
    base.require(isinstance(documents, dict) and set(documents) <= source_ids, '授权文件不得关联未知来源')
    packet = value['packet']
    sources = {source['id']: source for source in packet['sources']}
    authorizations = authorization_progress(packet['sources'], documents, path.parent)
    protocols_present = all(isinstance(packet[key], str) and base.SHA.fullmatch(packet[key]) for key in handoff.BINDINGS)
    rows, people = [], set()
    for task in packet['tasks']:
        source = sources.get(task['source_id'])
        source_matches = bool(source and task['source_binding_sha256'] == hashlib.sha256(handoff.canonical(source)).hexdigest())
        review = review_progress(task, packet, source_matches, protocols_present)
        complete = personal_complete(task)
        if not complete:
            review['issues'].insert(0, '个人标注字段未完整')
        authorization_ok = bool(source and authorizations[source['id']]['file_and_scope_consistent'])
        rows.append({'id': task['id'], 'personal_annotation_complete': complete, 'authorization_file_and_scope_consistent': authorization_ok,
                     'source_binding_consistent': source_matches, 'review': review,
                     'progress': 'current-review-declarations' if review['current_declarations_consistent'] else 'awaiting-review' if complete else 'pending-authoring'})
        actors = [task['author_id'], *(review.get('reviewer_id') for review in task['reviews']),
                  (task['adjudication'] or {}).get('adjudicator_id')]
        people.update(person for person in actors if isinstance(person, str) and person.strip())
    custodian = packet['custodian_id']
    custodian_consistent = isinstance(custodian, str) and bool(custodian.strip()) and custodian not in people
    return {'schema_version': 1, 'experiment': 'cloudleaf-authoring-progress-v1', 'usage': 'internal-development-only',
            'checked_at_utc': datetime.now(timezone.utc).isoformat(), 'queue_sha256': base.digest(path),
            'origin_packet_sha256': origin['packet_sha256'], 'tool_sha256': base.digest(Path(__file__)),
            'digest_contract_tool_sha256': base.digest(Path(handoff.__file__)), 'source_sha256': base.source_sha256(),
            'counts': {'tasks': len(rows), 'original_task_slots': len(origin['task_ids']),
                       'personal_annotations_complete': sum(row['personal_annotation_complete'] for row in rows),
                       'current_review_declarations_consistent': sum(row['review']['current_declarations_consistent'] for row in rows)},
            'protocol_sha_declarations_present': protocols_present, 'authorizations': authorizations,
            'custodian_declaration_consistent': custodian_consistent, 'tasks': rows,
            'formal_ready': False, 'language_accuracy_valid': False, 'top1': None, 'top5': None, 'mrr': None,
            'human_identity_or_signed_authorization_proven': False, 'additional_heldout_files_opened': False,
            'formal_blockers': ['作者草稿不具备正式gold身份，必须另走现有三个正式门禁。',
                                '真实授权、独立双审/仲裁、语义家族及heldout保管/封存仍须人工事实核验。'],
            'limitations': ['有效报告exit0只表示工具执行成功，不代表正式gold或语言质量通过。',
                            '不同ID和摘要相符仅为声明一致，不证明真实人员身份、独立性或实际签署。',
                            '不输出任务意图/拼音/答案；不要给本工具提供冻结或heldout文件。']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    export = commands.add_parser('export')
    export.add_argument('--packet', type=Path, required=True)
    export.add_argument('--sha256', required=True)
    export.add_argument('--output', type=Path, required=True)
    progress = commands.add_parser('status')
    progress.add_argument('--queue', type=Path, required=True)
    progress.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('输出已存在，禁止覆盖历史或个人草稿')
    try:
        value = export_queue(args.packet, args.sha256) if args.command == 'export' else status(args.queue)
    except (ValueError, KeyError, TypeError, OSError) as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps({'operation': args.command, 'output': str(args.output), 'usage': 'internal-development-only', 'formal_ready': False}, ensure_ascii=False))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

"""离线核验C到B的可见任务语义；不打开heldout答案，不计算语言指标。"""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path

import check_language_readiness as base
from check_quality_preparation import validate_preparation

FIELDS = ('id', 'source_id', 'source_unit_id', 'family_id', 'near_duplicate_group', 'split',
          'source_binding_sha256', 'author_id', 'input_intent', 'pinyin', 'acceptable_outputs', 'key_trace', 'scenario')
BINDINGS = ('protocol_sha256', 'preprocessing_sha256', 'lineage_audit_sha256')
METADATA = {'schema_version', 'format', 'synthetic', 'dictionary_derived', 'custodian_id', 'sources', 'task_ids', *BINDINGS}


def canonical(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')) + '\n').encode('utf-8')


def annotation_digest(task):
    return hashlib.sha256(canonical({key: task.get(key) for key in FIELDS})).hexdigest()


def review_binding_digest(metadata):
    return hashlib.sha256(canonical({key: metadata.get(key) for key in BINDINGS})).hexdigest()


def validate_handoff(value, plan, rows, approved, preparation, folder):
    base.require(value.get('schema_version') == 1 and value.get('experiment') == 'cloudleaf-ime-task-handoff-v1', '任务交付版本无效')
    base.require(value.get('family_index_sha256') == plan['family_index']['sha256'], '交付未绑定固定家族清单')
    metadata = base.read_json(base.checked(value['metadata'], folder))
    base.require(set(metadata) == METADATA and metadata['schema_version'] == 1
                 and metadata['format'] == 'cloudleaf-ime-annotation-v1', '公共元信息含正文/答案或协议字段不符')
    base.require(metadata['synthetic'] is False and metadata['dictionary_derived'] is False, '交付不是独立人工任务声明')
    base.require(isinstance(metadata['custodian_id'], str) and metadata['custodian_id'].strip(), '缺独立保管者声明')
    for name, document in zip(BINDINGS, ['annotation_protocol', 'preprocessing', 'lineage_audit']):
        base.require(metadata[name] == preparation[document]['sha256'], 'C与B实际协议/预处理/lineage审核不一致')
    binding = review_binding_digest(metadata)
    sources, lineages = {}, {source['id']: source['lineage_id'] for source in preparation['sources']}
    for source in metadata['sources']:
        key = source['id']
        base.require(key in approved and approved[key]['role'] == 'evaluation-corpus' and key not in sources, '任务来源未获批准或重复')
        base.require(source['artifact_sha256'] == approved[key]['artifact']['sha256']
                     and source['version'] == approved[key]['version'] and source['lineage_id'] == lineages[key], '任务原件/版本/lineage漂移')
        grant = source['authorization']
        base.require(grant.get('status') == 'approved' and isinstance(grant.get('sha256'), str)
                     and base.SHA.fullmatch(grant['sha256']) and 'evaluation' in grant.get('scope', []), '任务来源缺固定评估授权声明')
        sources[key] = source
    base.require(set(sources) == set(lineages), '任务元信息没有覆盖全部评估来源')
    index = {row['case_id']: row for row in rows}
    base.require(len(index) == len(rows) and len(metadata['task_ids']) == len(index)
                 and set(metadata['task_ids']) == set(index), '任务元信息与固定分母不符')
    context = base.read_json(base.checked(value['context_review'], folder))
    base.require(context.get('schema_version') == 1 and context.get('family_index_sha256') == value['family_index_sha256']
                 and context.get('review_binding_sha256') == binding, '上下文/意图审核未绑定固定任务及协议')
    groups, owners = {}, {}
    for row in context['all_case_intent_groups']:
        base.require(set(row) == {'case_id', 'intent_family_id'} and row['case_id'] in index
                     and row['case_id'] not in groups and isinstance(row['intent_family_id'], str)
                     and row['intent_family_id'].strip(), '公共意图组不得含正文/答案，且须唯一对应案例')
        split = index[row['case_id']]['split']
        base.require(owners.setdefault(row['intent_family_id'], split) == split, '同意图家族跨split泄漏')
        groups[row['case_id']] = row['intent_family_id']
    base.require(set(groups) == set(index), '意图审核未覆盖全部固定分母')
    visible_ids = {key for key, row in index.items() if row['split'] != 'heldout'}
    contexts = {}
    for row in context['cases']:
        key = row['case_id']
        base.require(set(row) == {'case_id', 'task_sha256', 'intent_family_id', 'context_kind', 'confirmed_left_context'}
                     and key in visible_ids and key not in contexts and row['task_sha256'] == index[key]['task_sha256']
                     and row['intent_family_id'] == groups[key], '上下文审核任务摘要/分母不符或包含heldout正文')
        left = row['confirmed_left_context']
        base.require(isinstance(left, str) and ((row['context_kind'] == 'none' and left == '')
                     or (row['context_kind'] == 'confirmed-left' and bool(left.strip()))), '上下文类型与明确左侧内容不一致')
        contexts[key] = row
    base.require(set(contexts) == visible_ids and set(value['visible_tasks']) == {'train', 'dev'}, '可见任务/上下文分母不符，不能打开heldout')
    seen, people = set(), set()
    for split, descriptor in value['visible_tasks'].items():
        path = base.checked(descriptor, folder)
        for line in path.read_text(encoding='utf-8').splitlines():
            task = json.loads(line, object_pairs_hook=base.no_duplicate_keys)
            key = task['id']
            base.require(key in visible_ids and key not in seen and task.get('status') == 'completed'
                         and task['split'] == split == index[key]['split'], '可见任务分母/状态/split不符')
            base.require(all(isinstance(task.get(field), str) and task[field].strip() for field in FIELDS
                             if field not in {'acceptable_outputs', 'key_trace'}), '任务意图/拼音/场景字段缺失')
            base.require(all(isinstance(task.get(field), list) and task[field]
                             and all(isinstance(text, str) and text.strip() for text in task[field])
                             for field in ['acceptable_outputs', 'key_trace']), '可接受输出/按键轨迹缺失')
            base.require(all(task[field] == index[key][field] for field in
                             ['source_id', 'source_unit_id', 'family_id', 'near_duplicate_group']), '任务与索引来源/家族不符')
            base.require(annotation_digest(task) == index[key]['task_sha256']
                         and base.normalized_digest(task['pinyin']) == index[key]['normalized_input_sha256'], '任务或规范输入摘要不符')
            base.require(task['source_binding_sha256'] == hashlib.sha256(canonical(sources[task['source_id']])).hexdigest(), '具体来源/授权绑定已变更')
            reviews = task['reviews']
            base.require(isinstance(reviews, list) and len(reviews) == 2, '需要两位独立审核员')
            reviewers = [review['reviewer_id'] for review in reviews]
            base.require(all(isinstance(person, str) and person.strip() for person in reviewers)
                         and len(set(reviewers)) == 2 and task['author_id'] not in reviewers, '作者与两位审核员不独立')
            base.require(all(review.get('annotation_sha256') == index[key]['task_sha256']
                             and review.get('protocol_sha256') == metadata['protocol_sha256']
                             and review.get('review_binding_sha256') == binding
                             and review.get('decision') in {'approve', 'reject'} for review in reviews), '审核已失效或未绑定完整三协议')
            people.update([task['author_id'], *reviewers])
            if any(review['decision'] != 'approve' for review in reviews):
                decision = task.get('adjudication') or {}
                arbiter = decision.get('adjudicator_id')
                base.require(isinstance(arbiter, str) and arbiter.strip() and arbiter not in {task['author_id'], *reviewers}
                             and decision.get('decision') == 'approve' and decision.get('annotation_sha256') == index[key]['task_sha256']
                             and decision.get('protocol_sha256') == metadata['protocol_sha256']
                             and decision.get('review_binding_sha256') == binding, '分歧未独立仲裁当前任务及三协议')
                people.add(arbiter)
            seen.add(key)
    base.require(seen == visible_ids, '可见任务丢失固定分母')
    base.require(metadata['custodian_id'] not in people, '保管者与可见任务人员不独立')
    return {'visible_tasks': len(seen), 'heldout_index_cases': len(index) - len(seen), 'review_binding_sha256': binding,
            'heldout_answers_opened': False, 'human_identity_or_semantic_groups_proven': False}


def run():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ['plan', 'preparation', 'handoff', 'output']:
        parser.add_argument('--' + key, type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('输出已存在，禁止覆盖历史证据')
    plan, preparation = base.read_json(args.plan), base.read_json(args.preparation)
    old, issues, details = base.audit(plan, args.plan.parent), [], None
    try:
        approved = base.sources(plan, args.plan.parent)
        rows, _ = base.split_index(plan, args.plan.parent, approved)
        validate_preparation(preparation, plan, rows, approved, args.preparation.parent)
        details = validate_handoff(base.read_json(args.handoff), plan, rows, approved, preparation, args.handoff.parent)
    except (ValueError, KeyError, TypeError, OSError) as error:
        issues.append(str(error))
    result = {'schema_version': 1, 'experiment': 'cloudleaf-ime-task-handoff-report-v1', 'base_readiness': old,
              'task_handoff': {'passed': not issues, 'details': details, 'issues': issues},
              'prerequisites_ready': old['prerequisites_ready'] and not issues,
              'language_accuracy_valid': False, 'top1': None, 'top5': None, 'mrr': None,
              'heldout_answers_opened_by_handoff': False, 'checked_at_utc': datetime.now(timezone.utc).isoformat(),
              'plan_sha256': base.digest(args.plan), 'preparation_sha256': base.digest(args.preparation),
              'handoff_sha256': base.digest(args.handoff), 'tool_sha256': base.digest(Path(__file__)),
              'source_sha256': base.source_sha256(),
              'limitations': ['只核可见任务及人工声明一致性，不证明真人/授权或语义近重复。',
                              'heldout摘要/分组不证明其正文或审核；加密封存/保管者和正式配额仍须独立审阅。']}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps({'prerequisites_ready': result['prerequisites_ready'], 'issues': issues}, ensure_ascii=False))
    if not result['prerequisites_ready']:
        raise SystemExit(2)


if __name__ == '__main__':
    run()

"""只核对来源lineage准备声明；基础门禁与人工真实性独立，不读取gold。"""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path

import check_language_readiness as base


def validate_preparation(value: dict, plan: dict, rows: list[dict], approved: dict, folder: Path) -> dict:
    base.require(value.get('schema_version') == 1 and value.get('experiment') == 'cloudleaf-quality-preparation-v1', '准备契约版本错误')
    base.require(value.get('normalization') == plan['normalization'] == 'ascii-lower-umlaut-v1', '准备与基础规范化协议不一致')
    base.require(value.get('family_index_sha256') == plan['family_index']['sha256'], '准备未绑定完整家族清单')
    bound = {key: base.digest(base.checked(value[key], folder)) for key in
             ['preprocessing', 'lineage_audit', 'annotation_protocol', 'annotation_review']}
    expected = {key for key, source in approved.items() if source['role'] == 'evaluation-corpus'}
    sources, artifact_lineages = {}, {}
    for source in value.get('sources', []):
        key = source['id']
        base.require(key in expected and key not in sources, '准备来源额外/重复/未获批准')
        base.require(source.get('artifact_sha256') == approved[key]['artifact']['sha256'], '准备来源原件摘要不一致')
        base.require(source.get('preprocessing_sha256') == bound['preprocessing'], '来源预处理协议不一致')
        base.require(source.get('annotation_protocol_sha256') == bound['annotation_protocol']
                     and source.get('annotation_review_sha256') == bound['annotation_review'], '来源标注协议/review绑定不一致')
        lineage = source.get('lineage_id')
        base.require(isinstance(lineage, str) and bool(lineage.strip()), '来源lineage缺失')
        base.require(artifact_lineages.setdefault(source['artifact_sha256'], lineage) == lineage, '同一原件换来源ID却声明不同lineage')
        sources[key] = source
    base.require(set(sources) == expected and bool(expected), '准备必须覆盖全部评估来源')
    review = base.read_json(base.checked(value['annotation_review'], folder))
    base.require(review.get('schema_version') == 1, '标注review版本错误')
    covered = {}
    for row in review.get('sources', []):
        key = row.get('id')
        base.require(key in expected and key not in covered
                     and row.get('artifact_sha256') == sources[key]['artifact_sha256']
                     and row.get('annotation_protocol_sha256') == bound['annotation_protocol'], '标注review来源/原件/协议覆盖不一致')
        covered[key] = row
    base.require(set(covered) == expected, '标注review没有覆盖全部评估来源')
    owners = {}
    for row in rows:
        base.require(row['source_id'] in sources, '家族行没有来源lineage')
        key = (sources[row['source_id']]['lineage_id'], row['source_unit_id'])
        base.require(owners.setdefault(key, row['split']) == row['split'], '同lineage规范段落跨split泄漏')
    base.require(bool(rows), '缺少实际家族行，不能仅凭来源声明放行')
    return {'evaluation_sources': len(sources), 'source_lineages': len(set(source['lineage_id'] for source in sources.values())),
            'canonical_lineage_units': len(owners), 'bound_documents_sha256': bound,
            'semantic_lineage_independently_proven': False, 'language_metrics_computed': False}


def run() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--preparation', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('输出已存在，禁止覆盖历史证据')
    plan = base.read_json(args.plan)
    old = base.audit(plan, args.plan.parent)
    issues = []
    details = None
    try:
        approved = base.sources(plan, args.plan.parent)
        rows, _ = base.split_index(plan, args.plan.parent, approved)
        details = validate_preparation(base.read_json(args.preparation), plan, rows, approved, args.preparation.parent)
    except (ValueError, KeyError, TypeError, OSError) as error:
        issues.append(str(error))
    report = {'schema_version': 1, 'experiment': 'cloudleaf-quality-preparation-report-v1',
              'base_readiness': old, 'preparation': {'passed': not issues, 'details': details, 'issues': issues},
              'prerequisites_ready': old['prerequisites_ready'] and not issues,
              'language_accuracy_valid': False, 'top1': None, 'top5': None, 'mrr': None,
              'plan_sha256': base.digest(args.plan), 'preparation_sha256': base.digest(args.preparation),
              'tool_sha256': base.digest(Path(__file__)), 'source_sha256': base.source_sha256(),
              'checked_at_utc': datetime.now(timezone.utc).isoformat(),
              'limitations': ['核对人工提供的lineage/规范段落/协议与SHA，不自动证明语义聚类、人工独立性、权限或保管者身份。',
                              '没有打开封存答案，不计算语言指标；一致的准备声明不等于真实评测或产品完成。']}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps({'output': str(args.output), 'prerequisites_ready': report['prerequisites_ready'], 'preparation_issues': issues}, ensure_ascii=False))
    if not report['prerequisites_ready']:
        raise SystemExit(2)


if __name__ == '__main__':
    run()

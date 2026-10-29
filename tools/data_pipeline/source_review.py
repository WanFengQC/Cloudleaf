"""重放既有来源审查和固定文件身份；不新增授权或推断未知权限。"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from .annotation_workflow import canonical, require


def build_matrix(root):
    evidence = {}

    def read(name):
        data = (root / name).read_bytes()
        evidence[name] = hashlib.sha256(data).hexdigest()
        return data.decode('utf-8')

    registry = [json.loads(row) for row in read('data/sources/source-review.jsonl').splitlines()]
    records = {row['id']: row for row in registry}
    require(len(records) == len(registry), '来源审查登记ID重复')
    locks = {}
    for path in sorted((root / 'data/sources/locks').glob('*.json')):
        name = path.relative_to(root).as_posix()
        lock = json.loads(read(name))
        key = lock['source_id']
        require(key in records and key not in locks, '来源锁未登记或重复')
        license_bytes = read(lock['license_file']).encode('utf-8')
        require(hashlib.sha256(license_bytes).hexdigest() == lock['license_sha256'], '许可原文字节与锁不一致')
        require(records[key]['license'] == lock['license_id'], '登记许可与来源锁不一致')
        if records[key].get('archive_sha256') is not None:
            require(records[key]['archive_sha256'] == lock['sha256'], '登记原件与来源锁SHA不一致')
        for item, expected in lock.get('mirror_evidence', {}).items():
            require(hashlib.sha256(read(item).encode('utf-8')).hexdigest() == expected, '来源审核证据SHA不一致')
        locks[key] = (name, lock)
    result = []
    for record in registry:
        name, lock = locks.get(record['id'], (None, {}))
        permissions = lock.get('redistribution', {})
        result.append({
            'id': record['id'], 'official_source_or_distributor': record['source'],
            'distribution_kind': 'fixed-third-party-mirror' if lock.get('mirror_repository') else
                                 ('locked-source' if lock else 'candidate-not-acquired'),
            'version': lock.get('version', record.get('version')),
            'acquired_at_utc': None, 'reviewed_on': record['reviewed_on'],
            'original_sha256': lock.get('sha256'), 'original_bytes': lock.get('bytes'),
            'full_original_sha256': lock.get('original_sha256', lock.get('sha256')),
            'source_lock': name, 'source_lock_sha256': evidence.get(name),
            'project_review_status': record['status'],
            'approved_scope': lock.get('approved_scope', record.get('scope')) if lock.get('review_status') == 'approved' else None,
            'research_only': lock.get('research_only', True if not lock else False),
            'license_id': lock.get('license_id', record.get('license')),
            'license_text_file': lock.get('license_file'), 'license_text_sha256': lock.get('license_sha256'),
            'license_text_verified': bool(lock),
            'permissions': {'commercial_use': permissions.get('commercial_use'),
                            'modification': permissions.get('modified_data'),
                            'modification_declaration': record.get('modification'),
                            'redistribution_declaration': record.get('redistribution')},
            'attribution': lock.get('attribution', record.get('attribution')),
            'page_or_content_exceptions': lock.get('limitations', record.get('blocker')),
            'processing_protocol': lock.get('transformations'), 'tokenizer_protocol': None,
            'statistics_ready': False, 'token_frequency': None, 'document_frequency': None, 'zipf': None,
            'existing_review_record': record,
        })
    gate_name = 'docs/reports/cl009/round6/wikipedia-gate.json'
    acquisition_name = 'docs/reports/cl009/round6/wikipedia-acquisition.json'
    gate = json.loads(read(gate_name))
    acquisition = json.loads(read(acquisition_name))
    revision = next(row for row in acquisition if row['id'] == 'revision')
    require(revision['sha256'] == gate['response_sha256'], '固定Wikipedia取得记录与准入原件不一致')
    result.append({'id': f'wikipedia:zh-page{gate["page_id"]}-rev{gate["revision_id"]}',
                   'official_source_or_distributor': revision['url'], 'distribution_kind': 'official-fixed-revision-candidate',
                   'version': str(gate['revision_id']), 'acquired_at_utc': revision['requested_at_utc'],
                   'original_sha256': revision['sha256'], 'original_bytes': revision['bytes'],
                   'full_original_sha256': revision['sha256'], 'content_sha256': gate['content_sha256'],
                   'project_review_status': 'blocked-rights-authors-extraction-tokenizer', 'approved_scope': None,
                   'research_only': True, 'license_id': None, 'license_text_file': None,
                   'license_text_sha256': None, 'license_text_verified': False,
                   'permissions': {'commercial_use': None, 'modification': None, 'redistribution_declaration': None},
                   'attribution': {'all_authors_resolved': gate['all_authors_resolved'], 'history_url': gate['history_url']},
                   'page_or_content_exceptions': gate['issues'], 'processing_protocol': None, 'tokenizer_protocol': None,
                   'statistics_ready': gate['statistical_use_ready'], 'token_frequency': None,
                   'document_frequency': None, 'zipf': None, 'prior_gate_snapshot': gate_name})
    return {'format': 'cloudleaf-source-review-matrix-v1', 'schema_version': 1,
            'creates_new_source_approval': False, 'language_accuracy_valid': False,
            'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'evidence_files_sha256': evidence, 'sources': result,
            'limitations': ['只重放已有审查和固定文件身份，不独立证明许可/来源/真人真实性。',
                            '许可商业用途声明不等于全部页面例外和统计用途已批准。',
                            '缺取得时间保持null；reviewed_on不是取得时间。',
                            '不改变既有来源锁、生产包或research_only范围，不生成新词频或语义事实。']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    require(not args.output.exists(), '禁止覆盖已存在来源审查矩阵')
    result = build_matrix(args.root)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(canonical(result))
    print(json.dumps({'sources': len(result['sources']), 'creates_new_source_approval': False}, ensure_ascii=False))


if __name__ == '__main__':
    main()

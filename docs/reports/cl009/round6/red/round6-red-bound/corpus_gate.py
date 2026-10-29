"""离线核对单个官方revision身份，权利和分词未齐时禁止统计采用。"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from .annotation_workflow import canonical, require


def audit_revision(response, expected, review):
    pages = response['query']['pages']
    require(len(pages) == 1, '必须是唯一固定页面')
    page = pages[0]
    require(page['pageid'] == expected['page_id'] and page['ns'] == 0 and len(page['revisions']) == 1, '页面/命名空间/版本不唯一')
    revision = page['revisions'][0]
    require(revision['revid'] == expected['revision_id'], 'revision身份不符')
    slot = revision['slots']['main']
    require(slot['contentmodel'] == 'wikitext' and isinstance(slot['content'], str), '不是完整wikitext正文')
    raw = slot['content'].encode('utf-8')
    require(hashlib.sha1(raw).hexdigest() == revision['sha1'], 'revision内容SHA1不符')
    issues = []
    for key in ['license_and_exception_review_sha256', 'attribution_history_sha256',
                'extraction_protocol_sha256', 'tokenizer_protocol_sha256']:
        value = review.get(key)
        if not isinstance(value, str) or len(value) != 64 or any(c not in '0123456789abcdef' for c in value):
            issues.append('missing:' + key)
    if review.get('all_page_exceptions_reviewed') is not True:
        issues.append('page-exceptions-unreviewed')
    if review.get('reviewed_revision_id') != revision['revid'] or review.get('reviewed_content_sha256') != hashlib.sha256(raw).hexdigest():
        issues.append('review-not-bound-to-content')
    return {'revision_identity_valid': True, 'page_id': page['pageid'], 'revision_id': revision['revid'],
            'revision_timestamp': revision['timestamp'], 'content_sha1': revision['sha1'],
            'content_sha256': hashlib.sha256(raw).hexdigest(), 'content_bytes': len(raw),
            'history_url': f'https://zh.wikipedia.org/w/index.php?curid={page["pageid"]}&action=history',
            'permalink': f'https://zh.wikipedia.org/w/index.php?oldid={revision["revid"]}',
            'statistical_use_ready': not issues, 'issues': issues, 'all_authors_resolved': False,
            'token_frequency': None, 'document_frequency': None, 'zipf': None,
            'body_included_in_report': False, 'formal_language_quality_valid': False,
            'limitations': ['末编辑者不是全部作者。文件声明不能证明许可/页面例外审读或分词真实性；百科不是日常输入分布。']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--revision', type=Path, required=True)
    parser.add_argument('--page-id', type=int, required=True)
    parser.add_argument('--revision-id', type=int, required=True)
    parser.add_argument('--review', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    require(not args.output.exists(), '禁止覆盖历史准入结果')
    result = audit_revision(json.loads(args.revision.read_text(encoding='utf-8')),
                            {'page_id': args.page_id, 'revision_id': args.revision_id},
                            json.loads(args.review.read_text(encoding='utf-8')) if args.review else {})
    result.update(response_sha256=hashlib.sha256(args.revision.read_bytes()).hexdigest(),
                  tool_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(canonical(result))
    print(json.dumps(result, ensure_ascii=False))
    if not result['statistical_use_ready']:
        raise SystemExit(2)


if __name__ == '__main__':
    main()

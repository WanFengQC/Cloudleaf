"""固定revision及统计准入不能由许可关键词或末编辑者代替。"""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from tools.data_pipeline.corpus_gate import audit_revision


def response():
    content = '自编原文{{引文}}'
    return {'query': {'pages': [{'pageid': 342, 'ns': 0, 'title': 'fixture',
                               'revisions': [{'revid': 9, 'timestamp': '2026-01-01T00:00:00Z',
                                              'sha1': hashlib.sha1(content.encode()).hexdigest(),
                                              'user': 'fixture-last-editor',
                                              'slots': {'main': {'contentmodel': 'wikitext', 'content': content}}}]}]}}


class CorpusGateTests(unittest.TestCase):
    def test_bound_documents_are_checked_and_changed_protocol_fails(self):
        value = response()
        content = value['query']['pages'][0]['revisions'][0]['slots']['main']['content']
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            rights = {'status': 'approved', 'reviewer_id': 'synthetic-reviewer', 'page_id': 342,
                      'revision_id': 9, 'content_sha256': hashlib.sha256(content.encode()).hexdigest(),
                      'license_id': 'CC-BY-SA-4.0', 'all_page_exceptions_reviewed': True}
            history = {'page_id': 342, 'revision_id': 9, 'all_authors_resolved': True,
                       'history_url': 'https://example.invalid/synthetic-history'}
            protocol = {'id': 'synthetic', 'version': '1', 'normalization': 'synthetic',
                        'implementation_sha256': 'a' * 64, 'configuration_sha256': 'b' * 64, 'unit': 'tokens'}
            review = {}
            for name, document in [('license_and_exception_review', rights), ('attribution_history', history),
                                   ('extraction_protocol', protocol), ('tokenizer_protocol', protocol)]:
                path = root / (name + '.json')
                path.write_text(json.dumps(document), encoding='utf-8')
                review[name] = {'path': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
            report = audit_revision(value, {'page_id': 342, 'revision_id': 9}, review, root)
            self.assertTrue(report['statistical_use_ready'])
            self.assertFalse(report['evidence_declarations_independently_proven'])
            (root / 'tokenizer_protocol.json').write_text('{}', encoding='utf-8')
            self.assertFalse(audit_revision(value, {'page_id': 342, 'revision_id': 9}, review, root)['statistical_use_ready'])

    def test_real_content_identity_does_not_approve_rights_or_tokenization(self):
        result = audit_revision(response(), {'page_id': 342, 'revision_id': 9}, {})
        self.assertTrue(result['revision_identity_valid'])
        self.assertFalse(result['statistical_use_ready'])
        self.assertIsNone(result['token_frequency'])
        self.assertFalse(result['all_authors_resolved'])
        self.assertNotIn('自编原文', str(result))

    def test_wrong_revision_and_content_hash_are_rejected(self):
        for kind in ['revision', 'sha1']:
            value = response()
            value['query']['pages'][0]['revisions'][0]['revid' if kind == 'revision' else 'sha1'] = 10 if kind == 'revision' else '0' * 40
            with self.assertRaises(ValueError):
                audit_revision(value, {'page_id': 342, 'revision_id': 9}, {})

    def test_claimed_hashes_without_real_bound_evidence_cannot_approve(self):
        value = response()
        content = value['query']['pages'][0]['revisions'][0]['slots']['main']['content']
        review = {key: 'a' * 64 for key in ['license_and_exception_review_sha256',
                  'attribution_history_sha256', 'extraction_protocol_sha256', 'tokenizer_protocol_sha256']}
        review.update(all_page_exceptions_reviewed=True, reviewed_revision_id=9,
                      reviewed_content_sha256=hashlib.sha256(content.encode()).hexdigest())
        self.assertFalse(audit_revision(value, {'page_id': 342, 'revision_id': 9}, review)['statistical_use_ready'])

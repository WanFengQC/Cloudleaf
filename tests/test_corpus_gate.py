"""固定revision及统计准入不能由许可关键词或末编辑者代替。"""
import hashlib
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

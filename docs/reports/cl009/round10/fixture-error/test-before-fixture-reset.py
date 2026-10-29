"""临时机械夹具的审批形状只测试代码，不是真实语义、人员、许可或签署。"""
import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from zipfile import ZipFile

from tools.data_pipeline import pipeline as p
from tools.data_pipeline import semantic_bundle as s


def fixture(root, name='input', previous=None, changed=False):
    folder = root / name
    folder.mkdir()
    source = root / (name + '.zip')
    license_path = root / (name + '-license')
    license_path.write_bytes(b'Mechanical fixture only, not actual data authorization.\n')
    with ZipFile(source, 'w') as archive:
        archive.writestr('Unihan_Readings.txt', 'U+4E00\tkMandarin\tyī\nU+4E8C\tkMandarin\tèr\n')
    lock = root / (name + '-lock.json')
    p.write_json(lock, {'schema_version': 1, 'review_status': 'approved', 'format': 'unihan-zip',
        'source_id': 'fixture-source', 'version': 'fixture-1', 'license_id': 'fixture-license',
        'sha256': p.digest(source.read_bytes()), 'license_sha256': p.digest(license_path.read_bytes()),
        'url': 'https://example.invalid/fixture', 'license_url': 'https://example.invalid/license',
        'limitations': ['Mechanical fixture, not actual rights.'], 'attribution': 'Cloudleaf tests', 'transformations': 'fixture'})
    catalog = folder / 'lexicon'
    p.build(lock, source, license_path, catalog)
    entries = [json.loads(line)['id'] for line in (catalog / 'records.jsonl').read_text(encoding='utf-8').splitlines()]
    (folder / 'license.txt').write_bytes(license_path.read_bytes())
    (folder / 'NOTICE.txt').write_bytes(b'Synthetic mechanical test declarations only.\n')
    rights = {'schema_version': 1, 'kind': 'cloudleaf-semantic-source-rights-v1', 'source_id': 'fixture-source',
              'version': 'fixture-1', 'artifact_sha256': p.digest(source.read_bytes()), 'license_id': 'fixture-license',
              'license_sha256': p.digest(license_path.read_bytes()), 'status': 'approved', 'research_only': False,
              'scope': ['entry-pos', 'sense', 'translation', 'phrase']}
    p.write_json(folder / 'rights.json', rights)
    protocols = {}
    for key in ['semantics', 'preprocessing', 'lineage']:
        (folder / (key + '.txt')).write_bytes(('fixture-' + key + '\n').encode('utf-8'))
        protocols[key] = s.descriptor(folder, key + '.txt')
    content = {'format': 'cloudleaf-semantic-content-v2', 'schema_version': 2,
        'catalogs': [{'namespace': 'fixture-zh', 'language': 'zh', 'path': 'lexicon',
                      'manifest_sha256': p.digest((catalog / 'manifest.json').read_bytes()),
                      'facts_sha256': p.digest((catalog / 'records.jsonl').read_bytes())}],
        'sources': [{'id': 'fixture-source', 'version': 'fixture-1', 'artifact_sha256': p.digest(source.read_bytes()),
                     'license_id': 'fixture-license', 'license': s.descriptor(folder, 'license.txt'),
                     'rights': s.descriptor(folder, 'rights.json'), 'status': 'approved',
                     'scope': ['entry-pos', 'sense', 'translation', 'phrase']}], 'protocols': protocols, 'records': []}
    assertion = [{'source_id': 'fixture-source', 'unit_id': 'fixture:unit', 'pointer': '/fixture',
                  'sha256': p.digest(b'fictional source assertion')}]
    content['records'] = [
        {'id': 'sense:fixture-one', 'kind': 'sense', 'status': 'approved', 'source_assertions': assertion,
         'payload': {'namespace': 'fixture-zh', 'entry_id': entries[0], 'language': 'zh',
                     'definition': '机械夹具修订' if changed else '机械夹具', 'pos_id': None}},
        {'id': 'sense:fixture-two', 'kind': 'sense', 'status': 'approved', 'source_assertions': assertion,
         'payload': {'namespace': 'fixture-zh', 'entry_id': entries[1], 'language': 'zh', 'definition': '第二机械夹具', 'pos_id': None}},
        {'id': 'relation:fixture', 'kind': 'translation', 'status': 'approved', 'source_assertions': assertion,
         'payload': {'source_sense_id': 'sense:fixture-one', 'target_sense_id': 'sense:fixture-two', 'relation': 'related'}}]
    migration = {'schema_version': 1, 'from_manifest_sha256': previous,
                 'mappings': [{'from_ids': ['sense:fixture-one'], 'to_ids': ['sense:fixture-one'], 'reason': 'fixture revision'}] if previous else []}
    write_content(folder, content, migration)
    return folder


def write_content(folder, content, migration=None):
    if migration is None:
        migration = p.read_json(folder / 'migration.json')
    p.write_json(folder / 'content.json', content)
    p.write_json(folder / 'migration.json', migration)
    binding = s.review_binding(content, migration)
    p.write_json(folder / 'reviews.json', {'schema_version': 1, 'review_binding_sha256': binding,
        'records': [{'record_id': row['id'], 'author_id': 'fixture-author',
            'decisions': [{'reviewer_id': actor, 'decision': 'approve', 'reason': 'fixture only', 'review_binding_sha256': binding}
                          for actor in ['fixture-reviewer-a', 'fixture-reviewer-b']], 'adjudication': None}
                    for row in content['records'] if row['status'] == 'approved']})


def trust(root, bundles, name='policy'):
    approvals = []
    for index, bundle in enumerate(bundles):
        manifest = p.read_json(bundle / 'manifest.json')
        approval = {'schema_version': 1, 'kind': 'cloudleaf-semantic-release-approval-v1',
                    'manifest_sha256': p.digest((bundle / 'manifest.json').read_bytes()),
                    'content_sha256': manifest['content_sha256'], 'review_binding_sha256': manifest['review_binding_sha256'],
                    'reader_version': 1, 'status': 'approved', 'authorized_by': 'fixture-project-owner',
                    'reason': 'Mechanical positive test, not a real approval.'}
        approval_path = root / f'{name}-approval-{index}.json'
        p.write_json(approval_path, approval)
        approvals.append({'manifest_sha256': approval['manifest_sha256'], 'content_sha256': manifest['content_sha256'],
            'review_binding_sha256': manifest['review_binding_sha256'], 'reader_version': 1,
            'approval': {'path': approval_path.name, 'sha256': p.digest(approval_path.read_bytes())},
            'sources': p.read_json(bundle / 'content.json')['sources']})
    path = root / (name + '.json')
    p.write_json(path, {'schema_version': 1, 'kind': 'cloudleaf-semantic-trust-policy-v1', 'releases': approvals})
    return {'path': str(path), 'sha256': p.digest(path.read_bytes())}


class SemanticBundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.input = fixture(self.root)
        self.bundle = self.root / 'bundle'

    def build(self):
        s.build_bundle(self.input, self.bundle)
        return self.bundle

    def test_reader_manifest_determinism_and_directed_lookup(self):
        bundle = self.build()
        other = self.root / 'bundle-copy'
        s.build_bundle(self.input, other)
        self.assertEqual((bundle / 'manifest.json').read_bytes(), (other / 'manifest.json').read_bytes())
        value = s.read_bundle(bundle)
        self.assertEqual(value['reverse_lookup']['sense:fixture-two'], ['relation:fixture'])
        self.assertNotIn('sense:fixture-one', value['reverse_lookup'])
        self.assertEqual(len(value['content']['records']), 3)
        report = s.verify_bundle(bundle, production=True, trust_policy=trust(self.root, [bundle]))
        self.assertTrue(report['external_policy_consistent'])
        self.assertFalse(report['cryptographic_signature_verified'])
        self.assertFalse(report['human_identity_or_rights_proven'])

    def test_pending_null_is_readable_but_never_production(self):
        content = p.read_json(self.input / 'content.json')
        content['records'][0].update(status='pending', payload=None)
        content['records'] = content['records'][:1]
        write_content(self.input, content)
        bundle = self.build()
        self.assertIsNone(s.read_bundle(bundle)['content']['records'][0]['payload'])
        with self.assertRaisesRegex(ValueError, 'pending|审核'):
            s.verify_bundle(bundle, production=True, trust_policy=trust(self.root, [bundle]))

    def test_missing_or_internal_policy_refuses_before_store_write(self):
        bundle = self.build()
        store = self.root / 'store'
        with self.assertRaises(ValueError):
            s.activate(bundle, store, trust_policy=None)
        self.assertFalse(store.exists())
        internal = bundle / 'policy.json'
        internal.write_bytes(b'{}')
        with self.assertRaises(ValueError):
            s.activate(bundle, store, trust_policy={'path': str(internal), 'sha256': p.digest(b'{}')})
        self.assertFalse(store.exists())

    def test_strict_schema_and_orphan_references(self):
        original = p.read_json(self.input / 'content.json')
        for mode in ['unknown', 'version', 'orphan-sense', 'orphan-entry', 'duplicate-id']:
            with self.subTest(mode=mode):
                content = copy.deepcopy(original)
                if mode == 'unknown': content['extra'] = True
                elif mode == 'version': content['schema_version'] = True
                elif mode == 'orphan-sense': content['records'][2]['payload']['target_sense_id'] = 'sense:absent'
                elif mode == 'orphan-entry': content['records'][0]['payload']['entry_id'] = 'entry:absent'
                else: content['records'][1]['id'] = content['records'][0]['id']
                write_content(self.input, content)
                with self.assertRaises(ValueError):
                    s.build_bundle(self.input, self.root / ('invalid-' + mode))

    def test_content_source_protocol_and_migration_changes_invalidate_review(self):
        original = p.read_json(self.input / 'content.json')
        for mode in ['content', 'source', 'protocol', 'migration']:
            with self.subTest(mode=mode):
                content = copy.deepcopy(original)
                write_content(self.input, original, {'schema_version': 1, 'from_manifest_sha256': None, 'mappings': []})
                if mode == 'content': content['records'][0]['payload']['definition'] += 'changed'
                elif mode == 'source': content['sources'][0]['status'] = 'research_only'
                elif mode == 'protocol':
                    (self.input / 'semantics.txt').write_bytes(b'changed protocol')
                    content['protocols']['semantics'] = s.descriptor(self.input, 'semantics.txt')
                else:
                    migration = p.read_json(self.input / 'migration.json')
                    migration['mappings'] = [{'from_ids': [], 'to_ids': ['sense:fixture-one'], 'reason': 'changed'}]
                    p.write_json(self.input / 'migration.json', migration)
                p.write_json(self.input / 'content.json', content)
                candidate = self.root / ('stale-' + mode)
                s.build_bundle(self.input, candidate)
                with self.assertRaises(ValueError):
                    s.verify_bundle(candidate, production=True, trust_policy=trust(self.root, [candidate], mode))

    def test_duplicate_json_keys_and_corruption_are_rejected(self):
        bundle = self.build()
        (bundle / 'NOTICE.txt').write_bytes(b'tampered')
        with self.assertRaises(ValueError): s.read_bundle(bundle)
        (self.input / 'content.json').write_bytes(b'{"schema_version":2,"schema_version":2}')
        with self.assertRaises(ValueError): s.build_bundle(self.input, self.root / 'duplicate')

    def test_upgrade_rollback_and_unmapped_revision(self):
        first = self.build()
        identity = p.digest((first / 'manifest.json').read_bytes())
        second_input = fixture(self.root, 'input-two', identity, changed=True)
        second = self.root / 'second'
        s.build_bundle(second_input, second)
        policy = trust(self.root, [first, second])
        store = self.root / 'store'
        first_state = s.activate(first, store, trust_policy=policy)
        user = store / 'user-dictionary.json'
        user.write_bytes(b'private user data')
        s.activate(second, store, trust_policy=policy)
        state = s.rollback(store, trust_policy=policy)
        self.assertEqual(state['current'], first_state['current'])
        self.assertEqual(user.read_bytes(), b'private user data')
        migration = p.read_json(second_input / 'migration.json')
        migration['mappings'] = []
        write_content(second_input, p.read_json(second_input / 'content.json'), migration)
        bad = self.root / 'bad-upgrade'
        s.build_bundle(second_input, bad)
        bad_policy = trust(self.root, [first, bad], 'bad-policy')
        before = (store / 'current.json').read_bytes()
        with self.assertRaisesRegex(ValueError, '迁移'):
            s.activate(bad, store, trust_policy=bad_policy)
        self.assertEqual((store / 'current.json').read_bytes(), before)

    def test_legacy_schema2_verifier_and_default_activation_remain_unchanged(self):
        manifest = p.verify_release(self.input / 'lexicon')
        self.assertEqual(manifest['schema_version'], 2)
        p.activate(self.input / 'lexicon', self.root / 'legacy-store')
        self.assertTrue((self.root / 'legacy-store/current.json').exists())


if __name__ == '__main__':
    unittest.main()

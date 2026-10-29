"""独立语义候选与外部审批清单约束的发布；不证明真人、权利或密码签名。"""
from __future__ import annotations

import argparse
import json
from pathlib import Path, PurePosixPath
import shutil

from . import pipeline as p
from .filesystem import assert_plain_path, assert_plain_tree

KINDS = {'entry-pos', 'sense', 'translation', 'phrase'}
STATUSES = {'pending', 'research_only', 'approved'}
LIMIT = 64 * 1024 * 1024


def require(value, message):
    if not value:
        raise ValueError(message)


def shape(value, keys):
    require(isinstance(value, dict) and set(value) == set(keys.split()), '语义schema字段集合不符')


def text(value):
    return isinstance(value, str) and 0 < len(value) <= 4096 and value == value.strip() and not any(ord(c) < 32 for c in value)


def hash_text(value):
    return isinstance(value, str) and p.SHA256.fullmatch(value) is not None


def unique_keys(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'JSON含重复键')
        result[key] = value
    return result


def read_json(path):
    assert_plain_path(path)
    require(path.is_file() and path.stat().st_size <= LIMIT, '语义JSON文件缺失或超过64MiB')
    raw = path.read_bytes()
    require(not raw.startswith(b'\xef\xbb\xbf'), '语义JSON不接受BOM')
    return json.loads(raw.decode('utf-8'), object_pairs_hook=unique_keys,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('JSON拒绝非有限数值')))


def resource_path(root, name):
    require(isinstance(name, str) and '\\' not in name and ':' not in name, '资源路径无效')
    relative = PurePosixPath(name)
    require(name == relative.as_posix() and relative.parts and not relative.is_absolute()
            and all(part not in {'.', '..'} for part in relative.parts), '资源路径越界')
    path = root.joinpath(*relative.parts)
    assert_plain_path(path)
    return path


def descriptor(root, name):
    path = resource_path(root, name)
    return {'path': name, 'sha256': p.digest(path.read_bytes())}


def checked_resource(root, item):
    shape(item, 'path sha256')
    require(hash_text(item['sha256']), '资源摘要格式错误')
    path = resource_path(root, item['path'])
    require(path.is_file() and path.stat().st_size <= p.MAX_RELEASE, '资源缺失或大小超限')
    require(p.digest(path.read_bytes()) == item['sha256'], '资源材料SHA不符')
    return path


def review_binding(content, migration):
    return p.digest(p.canonical({'content_sha256': p.digest(p.canonical(content)), 'migration_sha256': p.digest(p.canonical(migration))}))


def content_snapshot(root):
    content = read_json(root / 'content.json')
    shape(content, 'format schema_version catalogs sources protocols records')
    require(content['format'] == 'cloudleaf-semantic-content-v2' and type(content['schema_version']) is int
            and content['schema_version'] == 2, '语义内容版本不兼容')
    for key in ['catalogs', 'sources', 'records']:
        require(isinstance(content[key], list) and len(content[key]) <= 1_000_000, '语义集合数量或类型错误')
    resources = {'content.json', 'reviews.json', 'migration.json', 'NOTICE.txt'}
    entries, catalog_languages = {}, {}
    for catalog in content['catalogs']:
        shape(catalog, 'namespace language path manifest_sha256 facts_sha256')
        namespace = catalog['namespace']
        require(text(namespace) and namespace not in entries and text(catalog['language']), '词条目录命名空间重复或语言缺失')
        folder = resource_path(root, catalog['path'])
        p.verify_release(folder)
        require(p.digest((folder / 'manifest.json').read_bytes()) == catalog['manifest_sha256']
                and p.digest((folder / 'records.jsonl').read_bytes()) == catalog['facts_sha256'], '词条目录身份不符')
        entries[namespace] = {json.loads(line)['id'] for line in (folder / 'records.jsonl').read_text(encoding='utf-8').splitlines()}
        catalog_languages[namespace] = catalog['language']
        resources.update(path.relative_to(root).as_posix() for path in folder.iterdir())
    require(entries, '缺少实际词条目录')
    sources = {}
    for source in content['sources']:
        shape(source, 'id version artifact_sha256 license_id license rights status scope')
        require(all(text(source[key]) for key in ['id', 'version', 'license_id']) and source['id'] not in sources
                and hash_text(source['artifact_sha256']) and source['status'] in STATUSES, '来源身份或状态错误')
        require(isinstance(source['scope'], list) and len(source['scope']) == len(set(source['scope']))
                and all(kind in KINDS for kind in source['scope']), '来源范围错误')
        for field in ['license', 'rights']:
            path = checked_resource(root, source[field])
            resources.add(path.relative_to(root).as_posix())
        sources[source['id']] = source
    shape(content['protocols'], 'semantics preprocessing lineage')
    for document in content['protocols'].values():
        resources.add(checked_resource(root, document).relative_to(root).as_posix())
    records, reverse = {}, {}
    for record in content['records']:
        shape(record, 'id kind payload source_assertions status')
        require(text(record['id']) and record['id'] not in records and record['kind'] in KINDS
                and record['status'] in STATUSES, '语义ID重复、类型或状态错误')
        require(isinstance(record['source_assertions'], list) and record['source_assertions'], '缺来源断言')
        for assertion in record['source_assertions']:
            shape(assertion, 'source_id unit_id pointer sha256')
            require(assertion['source_id'] in sources and text(assertion['unit_id']) and text(assertion['pointer'])
                    and hash_text(assertion['sha256']), '来源断言或源引用错误')
        payload = record['payload']
        if payload is None:
            require(record['status'] != 'approved', '已审核记录不可缺payload')
        else:
            validate_payload(record['kind'], payload, entries, catalog_languages)
        records[record['id']] = record
    for record in records.values():
        payload = record['payload']
        if payload is None:
            continue
        if record['kind'] == 'sense' and payload['pos_id'] is not None:
            pos = records.get(payload['pos_id'])
            require(pos is not None and pos['kind'] == 'entry-pos' and pos['payload'] is not None
                    and all(pos['payload'][key] == payload[key] for key in ['namespace', 'entry_id']), '义项POS引用错误')
        if record['kind'] == 'translation':
            for key in ['source_sense_id', 'target_sense_id']:
                sense = records.get(payload[key])
                require(sense is not None and sense['kind'] == 'sense' and sense['payload'] is not None, '翻译义项引用错误')
            require(payload['source_sense_id'] != payload['target_sense_id'], '翻译不能自指')
            reverse.setdefault(payload['target_sense_id'], []).append(record['id'])
    migration = read_json(root / 'migration.json')
    shape(migration, 'schema_version from_manifest_sha256 mappings')
    require(type(migration['schema_version']) is int and migration['schema_version'] == 1
            and (migration['from_manifest_sha256'] is None or hash_text(migration['from_manifest_sha256']))
            and isinstance(migration['mappings'], list), '迁移格式错误')
    for row in migration['mappings']:
        shape(row, 'from_ids to_ids reason')
        require(all(isinstance(row[key], list) and len(row[key]) == len(set(row[key]))
                    and all(text(identity) for identity in row[key]) for key in ['from_ids', 'to_ids'])
                and (row['from_ids'] or row['to_ids']) and text(row['reason']), '迁移ID或理由错误')
        require(all(identity in records for identity in row['to_ids']), '迁移新ID缺失')
    reviews = read_json(root / 'reviews.json')
    shape(reviews, 'schema_version review_binding_sha256 records')
    require(type(reviews['schema_version']) is int and reviews['schema_version'] == 1
            and hash_text(reviews['review_binding_sha256']) and isinstance(reviews['records'], list), '审核材料版本错误')
    review_ids = set()
    for row in reviews['records']:
        shape(row, 'record_id author_id decisions adjudication')
        require(row['record_id'] in records and row['record_id'] not in review_ids
                and (row['author_id'] is None or text(row['author_id'])) and isinstance(row['decisions'], list), '审核对象或作者错误')
        review_ids.add(row['record_id'])
        for decision in row['decisions']:
            shape(decision, 'reviewer_id decision reason review_binding_sha256')
            require(text(decision['reviewer_id']) and decision['decision'] in {'approve', 'reject'} and text(decision['reason'])
                    and hash_text(decision['review_binding_sha256']), '审核声明字段错误')
        if row['adjudication'] is not None:
            shape(row['adjudication'], 'adjudicator_id decision reason review_binding_sha256')
            require(text(row['adjudication']['adjudicator_id']) and row['adjudication']['decision'] in {'approve', 'reject'}
                    and text(row['adjudication']['reason']) and hash_text(row['adjudication']['review_binding_sha256']), '仲裁声明字段错误')
    return {'content': content, 'reviews': reviews, 'migration': migration, 'records': records,
            'reverse_lookup': {key: sorted(ids) for key, ids in reverse.items()}, 'resources': resources}


def validate_payload(kind, value, entries, languages):
    if kind == 'translation':
        shape(value, 'source_sense_id target_sense_id relation')
        require(text(value['source_sense_id']) and text(value['target_sense_id'])
                and value['relation'] in {'equivalent', 'broader', 'narrower', 'related'}, '有向翻译类型错误')
        return
    fields = {'entry-pos': 'namespace entry_id tag tagset tagset_version',
              'sense': 'namespace entry_id language definition pos_id', 'phrase': 'namespace entry_id kind components'}
    shape(value, fields[kind])
    require(value['namespace'] in entries and value['entry_id'] in entries[value['namespace']], '语义entry引用缺失')
    if kind == 'entry-pos':
        require(all(text(value[key]) for key in ['tag', 'tagset', 'tagset_version']), 'POS缺版本')
    elif kind == 'sense':
        require(value['language'] == languages[value['namespace']] and text(value['definition'])
                and (value['pos_id'] is None or text(value['pos_id'])), '义项语言/定义错误')
    else:
        require(value['kind'] in {'idiom', 'collocation', 'multiword'} and isinstance(value['components'], list)
                and 1 <= len(value['components']) <= 64, '短语类型或组件错误')
        for component in value['components']:
            shape(component, 'namespace entry_id')
            require(component['namespace'] in entries and component['entry_id'] in entries[component['namespace']], '短语组件引用缺失')


def candidate_files(root):
    assert_plain_tree(root)
    return {path.relative_to(root).as_posix() for path in root.rglob('*') if path.is_file()}


def build_bundle(source, output):
    source, output = Path(source), Path(output)
    require(not output.exists(), '输出已存在，禁止覆盖')
    snapshot = content_snapshot(source)
    require(candidate_files(source) == snapshot['resources'], '候选文件清单含未知或缺失资源')
    resources = {name: {'bytes': (source / name).stat().st_size, 'sha256': p.digest((source / name).read_bytes())}
                 for name in sorted(snapshot['resources'])}
    require(sum(value['bytes'] for value in resources.values()) <= p.MAX_RELEASE, '语义bundle总量超限')
    manifest = {'format': 'cloudleaf-semantic-bundle-v1', 'schema_version': 1, 'reader_version': 1,
                'content_sha256': p.digest(p.canonical(snapshot['content'])),
                'review_binding_sha256': review_binding(snapshot['content'], snapshot['migration']),
                'previous_manifest_sha256': snapshot['migration']['from_manifest_sha256'], 'resources': resources,
                'builder_sha256': p.digest(Path(__file__).read_bytes())}
    shutil.copytree(source, output)
    p.write_json(output / 'manifest.json', manifest)
    return verify_bundle(output)


def read_bundle(root):
    root = Path(root)
    assert_plain_tree(root)
    manifest = read_json(root / 'manifest.json')
    shape(manifest, 'format schema_version reader_version content_sha256 review_binding_sha256 previous_manifest_sha256 resources builder_sha256')
    require(manifest['format'] == 'cloudleaf-semantic-bundle-v1' and type(manifest['schema_version']) is int
            and manifest['schema_version'] == 1 and type(manifest['reader_version']) is int and manifest['reader_version'] == 1,
            'bundle或reader版本不兼容')
    require(all(hash_text(manifest[key]) for key in ['content_sha256', 'review_binding_sha256', 'builder_sha256'])
            and isinstance(manifest['resources'], dict), '清单摘要或资源错误')
    total = 0
    for name, value in manifest['resources'].items():
        shape(value, 'bytes sha256')
        require(type(value['bytes']) is int and 0 <= value['bytes'] <= p.MAX_RELEASE and hash_text(value['sha256']), '资源大小/摘要错误')
        path = resource_path(root, name)
        require(path.is_file() and path.stat().st_size == value['bytes'] and p.digest(path.read_bytes()) == value['sha256'], 'bundle资源损坏')
        total += value['bytes']
    require(total <= p.MAX_RELEASE and candidate_files(root) == set(manifest['resources']) | {'manifest.json'}, 'bundle总量或完整清单不符')
    snapshot = content_snapshot(root)
    require(snapshot['resources'] == set(manifest['resources']) and p.digest(p.canonical(snapshot['content'])) == manifest['content_sha256']
            and review_binding(snapshot['content'], snapshot['migration']) == manifest['review_binding_sha256']
            and snapshot['migration']['from_manifest_sha256'] == manifest['previous_manifest_sha256'], '内容/协议/迁移身份不符')
    snapshot['manifest'] = manifest
    snapshot['manifest_sha256'] = p.digest((root / 'manifest.json').read_bytes())
    return snapshot


def production_blockers(root, snapshot):
    issues = []
    content = snapshot['content']
    sources = {source['id']: source for source in content['sources']}
    for source in sources.values():
        rights = read_json(checked_resource(root, source['rights']))
        shape(rights, 'schema_version kind source_id version artifact_sha256 license_id license_sha256 status research_only scope')
        matching = (rights['schema_version'] == 1 and type(rights['schema_version']) is int
            and rights['kind'] == 'cloudleaf-semantic-source-rights-v1'
            and rights['source_id'] == source['id'] and rights['version'] == source['version']
            and rights['artifact_sha256'] == source['artifact_sha256'] and rights['license_id'] == source['license_id']
            and rights['license_sha256'] == source['license']['sha256'] and rights['scope'] == source['scope'])
        if not matching or source['status'] != 'approved' or rights['status'] != 'approved' or rights['research_only'] is not False:
            issues.append('来源权利或范围材料未批准/不符:' + source['id'])
    binding = snapshot['manifest']['review_binding_sha256']
    reviews = {row['record_id']: row for row in snapshot['reviews']['records']}
    if snapshot['reviews']['review_binding_sha256'] != binding:
        issues.append('内容审核已失效，须重新真实审核')
    if not snapshot['records']:
        issues.append('没有已审语义内容')
    for record in snapshot['records'].values():
        if record['status'] != 'approved' or record['payload'] is None:
            issues.append('pending或未审核内容:' + record['id'])
        if any(sources[row['source_id']]['status'] != 'approved' or record['kind'] not in sources[row['source_id']]['scope'] for row in record['source_assertions']):
            issues.append('来源未覆盖内容范围:' + record['id'])
        review = reviews.get(record['id'])
        if review is None:
            issues.append('缺实际审核材料:' + record['id'])
            continue
        people = [row['reviewer_id'] for row in review['decisions']]
        if (not text(review['author_id']) or len(people) != 2 or len(set(people)) != 2 or review['author_id'] in people
                or any(row['review_binding_sha256'] != binding for row in review['decisions'])):
            issues.append('作者/双审独立性或绑定不符:' + record['id'])
        if any(row['decision'] == 'reject' for row in review['decisions']):
            decision = review['adjudication']
            if (decision is None or decision['decision'] != 'approve' or decision['adjudicator_id'] in [review['author_id'], *people]
                    or decision['review_binding_sha256'] != binding):
                issues.append('缺独立且当前的仲裁:' + record['id'])
    return issues


def external_document(reference, base, forbidden):
    shape(reference, 'path sha256')
    require(isinstance(reference['path'], str) and hash_text(reference['sha256']), '缺外部策略或审批文件')
    path = base / reference['path']
    assert_plain_path(path)
    path = path.resolve()
    require(not any(path.is_relative_to(folder.resolve()) for folder in forbidden), '外部策略/审批不能来自bundle或store内部')
    require(path.is_file() and path.stat().st_size <= LIMIT and p.digest(path.read_bytes()) == reference['sha256'], '外部策略或审批SHA不符')
    return read_json(path), path


def verify_policy(root, snapshot, policy, store=None):
    require(policy is not None, '缺显式外部受信策略；拒绝生产发布')
    forbidden = [root] + ([Path(store)] if store is not None else [])
    value, path = external_document(policy, Path.cwd(), forbidden)
    shape(value, 'schema_version kind releases')
    require(type(value['schema_version']) is int and value['schema_version'] == 1
            and value['kind'] == 'cloudleaf-semantic-trust-policy-v1' and isinstance(value['releases'], list), '外部策略版本不符')
    matches = []
    for release in value['releases']:
        shape(release, 'manifest_sha256 content_sha256 review_binding_sha256 reader_version approval sources')
        require(all(hash_text(release[key]) for key in ['manifest_sha256', 'content_sha256', 'review_binding_sha256'])
                and type(release['reader_version']) is int and release['reader_version'] == 1 and isinstance(release['sources'], list), '策略允许项格式错误')
        if release['manifest_sha256'] == snapshot['manifest_sha256']:
            matches.append(release)
    require(len(matches) == 1, '没有唯一的精确manifest外部批准项')
    release = matches[0]
    require(all(release[key] == snapshot['manifest'][key] for key in ['content_sha256', 'review_binding_sha256', 'reader_version'])
            and release['sources'] == snapshot['content']['sources'], '策略内容/审核/来源范围漂移')
    approval, _ = external_document(release['approval'], path.parent, forbidden)
    shape(approval, 'schema_version kind manifest_sha256 content_sha256 review_binding_sha256 reader_version status authorized_by reason')
    require(type(approval['schema_version']) is int and approval['schema_version'] == 1
            and type(approval['reader_version']) is int
            and approval['kind'] == 'cloudleaf-semantic-release-approval-v1' and approval['status'] == 'approved'
            and text(approval['authorized_by']) and text(approval['reason'])
            and all(approval[key] == release[key] for key in ['manifest_sha256', 'content_sha256', 'review_binding_sha256', 'reader_version']), '外部审批身份/内容绑定不符')


def verify_bundle(root, *, production=False, trust_policy=None, store=None):
    root = Path(root)
    snapshot = read_bundle(root)
    issues = production_blockers(root, snapshot)
    if production:
        require(not issues, '生产审核门禁拒绝:' + ';'.join(issues))
        verify_policy(root, snapshot, trust_policy, store)
    return {'format': 'cloudleaf-semantic-verification-v1', 'candidate_structure_consistent': True,
            'production_requested': production, 'production_blockers': issues, 'external_policy_consistent': bool(production),
            'manifest_sha256': snapshot['manifest_sha256'], 'records': len(snapshot['records']),
            'cryptographic_signature_verified': False, 'human_identity_or_rights_proven': False,
            'default_input_engine_changed': False, 'manifest': snapshot['manifest']}


def check_transition(previous, release):
    current = read_bundle(release)
    if previous is None:
        require(current['manifest']['previous_manifest_sha256'] is None, '首次发布不能声明不在store中的上一版')
        return
    old = read_bundle(previous)
    if old['manifest_sha256'] == current['manifest_sha256']:
        return
    require(current['manifest']['previous_manifest_sha256'] == old['manifest_sha256'], '升级未绑定实际上一版')
    before, after = old['records'], current['records']
    changed = {key for key in before.keys() | after.keys() if before.get(key) != after.get(key)}
    covered = set()
    for mapping in current['migration']['mappings']:
        require(set(mapping['from_ids']) <= before.keys() and set(mapping['to_ids']) <= after.keys(), '迁移存在孤儿ID')
        covered.update(mapping['from_ids'])
        covered.update(mapping['to_ids'])
    require(changed <= covered, '变化/删除/新增语义ID缺显式迁移')


def publisher_verifier(trust_policy, store):
    def verify(root):
        return verify_bundle(root, production=True, trust_policy=trust_policy, store=store)
    return verify


def check_store_policy(trust_policy, store):
    require(trust_policy is not None, '缺显式外部策略；拒绝store写入')
    value, _ = external_document(trust_policy, Path.cwd(), [Path(store)])
    shape(value, 'schema_version kind releases')
    require(type(value['schema_version']) is int and value['schema_version'] == 1
            and value['kind'] == 'cloudleaf-semantic-trust-policy-v1' and isinstance(value['releases'], list), '外部策略版本不符')


def activate(release, store, *, trust_policy):
    return p.activate(Path(release), Path(store), verifier=publisher_verifier(trust_policy, store), transition=check_transition)


def rollback(store, *, trust_policy):
    check_store_policy(trust_policy, store)
    return p.rollback(Path(store), verifier=publisher_verifier(trust_policy, store))


def recover(store, owner_pid, *, trust_policy, rollback=False):
    from .recovery import recover_store
    check_store_policy(trust_policy, store)
    return recover_store(Path(store), owner_pid, rollback=rollback, verifier=publisher_verifier(trust_policy, store))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    build = commands.add_parser('build')
    build.add_argument('--source', type=Path, required=True)
    build.add_argument('--output', type=Path, required=True)
    verify = commands.add_parser('verify')
    verify.add_argument('--bundle', type=Path, required=True)
    verify.add_argument('--production', action='store_true')
    publish = commands.add_parser('activate')
    publish.add_argument('--bundle', type=Path, required=True)
    for name in ['rollback', 'recover']:
        sub = commands.add_parser(name)
        sub.add_argument('--store', type=Path, required=True)
        if name == 'recover':
            sub.add_argument('--owner-pid', type=int, required=True)
            sub.add_argument('--rollback', action='store_true')
    publish.add_argument('--store', type=Path, required=True)
    for sub in [verify, publish, commands.choices['rollback'], commands.choices['recover']]:
        sub.add_argument('--trust-policy', type=Path)
        sub.add_argument('--trust-policy-sha256')
    args = parser.parse_args()
    policy = {'path': str(args.trust_policy), 'sha256': args.trust_policy_sha256} if getattr(args, 'trust_policy', None) else None
    try:
        if args.command == 'build': result = build_bundle(args.source, args.output)
        elif args.command == 'verify': result = verify_bundle(args.bundle, production=args.production, trust_policy=policy)
        elif args.command == 'activate': result = activate(args.bundle, args.store, trust_policy=policy)
        elif args.command == 'rollback': result = rollback(args.store, trust_policy=policy)
        else: result = recover(args.store, args.owner_pid, trust_policy=policy, rollback=args.rollback)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    print(json.dumps(result, ensure_ascii=False))


if __name__ == '__main__':
    main()

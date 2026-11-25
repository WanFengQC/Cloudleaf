"""以锁定的现有本地模型进行可追溯推理和拒绝路径测试。"""

import argparse
import json
import re
import shutil
from pathlib import Path

from r21_evidence_common import (
    LOCAL, ROOT, R13, TRANSLATION, get_json, git, head, identity, now, percentile, put_json, run,
)

PINNED_CATALOG = 'f88a5f57513a0aaae3bbd659950adf042c8c8dde07736084cb951f61f22c7f0f'
DEFAULT_CATALOG = TRANSLATION / '.local/round13-managed-models' / f'catalog-{PINNED_CATALOG}/catalog.json'


def approved_catalog(path):
    """原始许可、锁定权重、各资源哈希必须与原 Round 13 签入身份相符。"""
    evidence = {'catalog': identity(path), 'models': [], 'lock_sources': {}}
    assert evidence['catalog']['sha256'] == PINNED_CATALOG
    locks = {}
    for name in ('opus-source-lock.json', 'opus-converted-lock.json'):
        rel = f'config/offline-models/{name}'
        file = ROOT / rel
        raw = git('show', f'{R13}:{rel}')
        assert file.read_bytes() == raw
        locks[name] = json.loads(raw)
        evidence['lock_sources'][name] = identity(file)
    converted = {v['model_id']: v for v in locks['opus-converted-lock.json']['models']}
    source = {v['model_id']: v for v in locks['opus-source-lock.json']['models']}
    assert converted.keys() == source.keys() == {'opus-mt-en-zh', 'opus-mt-zh-en'}
    for reference in get_json(path)['models']:
        manifest_path = path.parent / reference['manifest']
        assert manifest_path.resolve().is_relative_to(path.parent.resolve())
        manifest_id = identity(manifest_path)
        assert manifest_id['sha256'] == reference['sha256']
        manifest = get_json(manifest_path)
        model_id = manifest['model_id']
        assert manifest['license_id'] == 'CC-BY-4.0'
        assert manifest['license_status'] == 'research_only'
        assert manifest['revision'] == converted[model_id]['revision'] == source[model_id]['revision']
        disk = {}
        for item in manifest['files']:
            path_checked = manifest_path.parent / item['path']
            assert path_checked.resolve().is_relative_to(manifest_path.parent.resolve())
            actual = identity(path_checked)
            assert actual['bytes'] == item['bytes'] and actual['sha256'] == item['sha256']
            disk[item['role']] = actual
        assert disk['weights']['sha256'] == converted[model_id]['sha256']
        assert disk['weights']['bytes'] == converted[model_id]['bytes']
        evidence['models'].append({
            'model_id': model_id,
            'direction': manifest['direction'],
            'license_id': manifest['license_id'],
            'license_status': manifest['license_status'],
            'revision': manifest['revision'],
            'manifest': manifest_id,
            'resources': disk,
        })
    assert {v['direction'] for v in evidence['models']} == {'zh-en', 'en-zh'}
    return evidence


def samples():
    pinned = ROOT / 'tests/fixtures/translation-offline-smoke.jsonl'
    assert pinned.read_bytes() == git('show', f'{R13}:tests/fixtures/translation-offline-smoke.jsonl')
    records = [json.loads(line) for line in pinned.read_text(encoding='utf-8').splitlines()]
    for ident, direction, text, extra in [
        ('two-sentences-zh', 'zh-en', '今天没下雨。我们准备出门。', {}),
        ('two-sentences-en', 'en-zh', 'It did not rain today. We are ready to leave.', {}),
        ('units-zh', 'zh-en', 'Cloudleaf 测试组在2026年11月25日记录了 12.5 公斤和 38 摄氏度。', {}),
        ('units-en', 'en-zh', 'On 2026-11-25, Cloudleaf recorded 12.5 kg at 38 degrees Celsius.', {}),
        ('long-zh', 'zh-en',
         '我们先核对文件，再检查记录中的日期和数量，最后通知负责处理的人员。如果发现编号不一致，请把原始记录和补充说明分别保存，以便下次复查。', {}),
        ('long-en', 'en-zh',
         'First check the file, then verify the dates and quantities in the records, and finally notify the person in charge. If an identifier differs, retain both the original record and the explanation for review.', {}),
        ('empty', 'en-zh', '', {}),
        ('blank', 'zh-en', ' \n\t ', {}),
        ('bytes-over-limit', 'en-zh', 'x' * 16385, {}),
        ('token-over-limit', 'en-zh', 'This sentence exceeds the available token budget. ' * 80, {}),
        ('pre-cancelled', 'en-zh', 'Hello.', {'cancel': True}),
        ('deadline-zero', 'zh-en', '你好。', {'timeout_seconds': 0}),
        ('recover-en', 'en-zh', 'Hello.', {}),
        ('recover-zh', 'zh-en', '你好。', {}),
    ]:
        records.append({'id': ident, 'direction': direction, 'text': text,
                        'origin': 'authored_r21_engineering_probe', **extra})
    return records


def faults(catalog, out):
    """以临时副本验证损坏哈希。复制一个小型模型配置，不复制权重。"""
    damaged = out / 'damaged-model'
    damaged.mkdir()
    value = get_json(catalog)
    first = value['models'][0]
    source_manifest = catalog.parent / first['manifest']
    manifest = get_json(source_manifest)
    config = next(file for file in manifest['files'] if file['role'] == 'config')
    manifest['files'].remove(config)
    manifest['files'].insert(0, config)
    manifest_path = damaged / first['manifest']
    manifest_path.parent.mkdir()
    content = bytearray((source_manifest.parent / config['path']).read_bytes())
    content[-2] ^= 1
    (manifest_path.parent / config['path']).write_bytes(content)
    put_json(manifest_path, manifest)
    first['sha256'] = identity(manifest_path)['sha256']
    value['models'] = [first]
    path = damaged / 'catalog.json'
    put_json(path, value)
    return path


def audit_model_rows(case_list, stdout):
    output = [json.loads(line) for line in stdout.read_text(encoding='utf-8').splitlines()]
    assert output.pop(0)['event'] == 'loaded'
    assert len(output) == len(case_list)
    faults = {'empty': 'InvalidInput', 'blank': 'InvalidInput', 'bytes-over-limit': 'InputTooLong',
              'token-over-limit': 'InputTooLong', 'pre-cancelled': 'Cancelled', 'deadline-zero': 'Timeout'}
    mismatches = []
    for i, (case, row) in enumerate(zip(case_list, output, strict=True)):
        assert row['index'] == i and row['input'] == case['text'] and row['direction'] == case['direction']
        if case['id'] in faults:
            assert row.get('error') == faults[case['id']], (case['id'], row.get('error'))
        else:
            assert row['quality'] == 'Generated' and row['output'].strip()
            source_numbers = re.findall(r'\d+(?:[.,]\d+)?', row['input'])
            if source_numbers and any(n not in row['output'] for n in source_numbers):
                mismatches.append({'id': case['id'], 'input': row['input'],
                                   'unmatched_literal_numbers': [n for n in source_numbers if n not in row['output']],
                                   'output': row['output']})
    return {'cases': len(output), 'generated': len(output) - len(faults), 'negative_cases': faults,
            'numeric_literal_differences': mismatches,
            'numeric_probe_note': 'Observed missing original numeral literals only; dates, units and semantic omissions need independent language review.',
            'formal_language_quality': None}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('release_dir', type=Path)
    parser.add_argument('--catalog', type=Path, default=DEFAULT_CATALOG)
    parser.add_argument('--label', default='round21-real-models')
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    out = LOCAL / args.label
    out.mkdir(exist_ok=False)
    material = approved_catalog(args.catalog)
    record = {'started_utc': now(), 'head': head(), 'model_source_head': R13, 'models': material,
              'tool': identity(__file__), 'common': identity(LOCAL / 'r21_evidence_common.py'),
              'network_scope': 'Only verified local model provider was invoked. Host network was not disconnected.',
              'source': {p.relative_to(ROOT).as_posix(): identity(p)['sha256']
                         for crate in ('cloudleaf-offline', 'cloudleaf-translation')
                         for p in (ROOT / 'crates' / crate).rglob('*.rs')},
              'runs': []}
    put_json(out / 'summary.json', record)
    if args.verify_only:
        print('Both existing catalog entries and every resource verified against Round 13 hashes.')
        return
    release = args.release_dir.resolve()
    offline, models, lifecycle = (release / 'examples' / f'{name}.exe'
                                  for name in ('offline_translate', 'offline_models', 'offline_lifecycle'))
    record['binaries'] = {p.stem: identity(p) for p in (offline, models, lifecycle)}
    case_list = samples()
    payload = ''.join(json.dumps(v, ensure_ascii=False) + '\n' for v in case_list).encode('utf-8')
    (out / 'samples.jsonl').write_bytes(payload)
    broken = faults(args.catalog, out)
    missing = out / 'not-installed' / 'catalog.json'
    for name, exe, arguments, data, expected in [
        ('catalog-verified', models, ['check', args.catalog], None, 0),
        ('missing-catalog', models, ['check', missing], None, 2),
        ('damaged-config', models, ['check', broken], None, 2),
        ('translate-real', offline, [args.catalog], payload, 0),
        ('active-cancel-queue-lifecycle', lifecycle, [args.catalog], None, 0),
    ]:
        folder = out / name
        result = run(folder, [exe, *arguments], payload=data, expected=expected, deadline_seconds=180)
        record['runs'].append(result)
        put_json(out / 'summary.json', record)
        rows = folder / 'stdout.txt'
        if name == 'translate-real':
            record['engineering_inference'] = audit_model_rows(case_list, rows)
        elif name == 'active-cancel-queue-lifecycle':
            data = [json.loads(line) for line in rows.read_text(encoding='utf-8').splitlines()]
            completed = data[-1]
            assert completed['event'] == 'passed'
            assert completed['maximum_active_provider_calls'] == 1
            assert completed['active_provider_calls_at_end'] == 0
            record['lifecycle'] = completed
        else:
            actual = get_json(rows)
            assert actual.get('error') == (None if expected == 0 else
                                          'ModelNotFound' if name == 'missing-catalog' else 'ModelLoadFailed')
    final = approved_catalog(args.catalog)
    assert final == material
    record['catalog_unchanged'] = True
    record['finished_utc'] = now()
    put_json(out / 'summary.json', record)
    print(json.dumps({'cases': len(case_list), 'runs': len(record['runs']),
                      'generated': record['engineering_inference']['generated'],
                      'literal_differences': len(record['engineering_inference']['numeric_literal_differences'])}))


if __name__ == '__main__':
    main()

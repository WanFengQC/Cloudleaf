"""只读核对隔离长测材料，不启动测试、不签发授权、不操作TSF。"""
from __future__ import annotations

import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        raw = args.request.read_bytes()
        result = check_request(json.loads(raw), args.request.resolve().parent)
        result['request_file'] = {'path': str(args.request.resolve()), 'bytes': len(raw),
                                  'sha256': hashlib.sha256(raw).hexdigest()}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open('x', encoding='utf-8', newline='\n') as output:
            json.dump(result, output, ensure_ascii=False, indent=2)
            output.write('\n')
    except (OSError, ValueError, TypeError) as error:
        parser.error(str(error))
    print(json.dumps({'ready': result['ready'], 'issues': len(result['issues']), 'child_started': False}, ensure_ascii=False))
    return 0 if result['ready'] else 2


def admission_report(issues, files):
    return {'schema_version': 2, 'kind': 'cloudleaf-windows-soak-material-check', 'ready': False,
            'materials_consistent': not issues, 'execution_identity_binding_checked': False,
            'issues': issues, 'checked_files': files, 'declarations_independently_verified': False,
            'child_started': False, 'system_operations_executed': False,
            'is_execution_authorization': False, 'long_term_acceptance': False}


def execution_identity_sha256(identity):
    # 批准摘要在准入通过后才加入运行身份，排除它避免授权文件自引用。
    value = {key: item for key, item in identity.items() if key != 'execution_approval_sha256'}
    raw = json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode('utf-8')
    return hashlib.sha256(raw).hexdigest()


def text_fields(value, names, label, issues):
    for name in names:
        if not isinstance(value.get(name), str) or not value[name].strip():
            issues.append(f'{label}.{name}:缺少非空文本')


def sha256_text(value):
    return isinstance(value, str) and re.fullmatch(r'[0-9a-fA-F]{64}', value) is not None


def timestamp(value, label, issues):
    try:
        if not isinstance(value, str) or datetime.fromisoformat(value).utcoffset() is None:
            raise ValueError('时间需要明确时区')
    except ValueError:
        issues.append(f'{label}:需要含时区ISO8601实际执行时间')


def read_evidence(reference, base, label, issues, files):
    if (not isinstance(reference, dict) or not isinstance(reference.get('path'), str)
            or not reference['path'].strip() or not sha256_text(reference.get('sha256'))):
        issues.append(f'{label}:缺少实际path和SHA-256')
        return None, None
    try:
        path = (base / reference['path']).resolve(strict=True)
        raw = path.read_bytes()
        actual = hashlib.sha256(raw).hexdigest()
        files.append({'field': label, 'path': str(path), 'bytes': len(raw), 'sha256': actual})
        if actual != reference['sha256'].lower():
            issues.append(f'{label}:文件SHA-256不符')
            return None, path
        value = json.loads(raw)
        if not isinstance(value, dict):
            raise ValueError('材料必须为JSON对象')
        return value, path
    except (OSError, ValueError) as error:
        issues.append(f'{label}:{error}')
        return None, None


def check_request(request, base, identity=None):
    issues, files = [], []
    if not isinstance(request, dict):
        return admission_report(['环境请求必须为JSON对象'], files)
    if request.get('schema_version') != 1 or request.get('kind') != 'cloudleaf-windows-external-acceptance-request':
        issues.append('环境请求schema/kind不符')
    if request.get('status') != 'ready_for_preflight' or request.get('user_reported_environment_status') != 'prepared':
        issues.append('隔离环境请求尚未准备完成')
    if request.get('is_execution_authorization') is not False or request.get('is_tsf_registration_authorization') is not False:
        issues.append('环境请求表不是执行或TSF授权')
    environment = request.get('environment')
    people = request.get('people')
    if not isinstance(environment, dict) or not isinstance(people, dict):
        return admission_report(issues + ['environment/people必须为对象'], files)
    text_fields(environment, ['vm_id', 'windows_version_and_build', 'architecture', 'snapshot_id'], 'environment', issues)
    text_fields(people, ['environment_owner', 'test_operator'], 'people', issues)
    if not sha256_text(environment.get('snapshot_storage_sha256')):
        issues.append('environment.snapshot_storage_sha256:缺少快照SHA-256')
    restore, _ = read_evidence(environment.get('restore_exercise_evidence'), base, 'restore_exercise_evidence', issues, files)
    if restore is not None:
        if restore.get('schema_version') != 1 or restore.get('kind') != 'cloudleaf-windows-restore-exercise' or restore.get('status') != 'passed':
            issues.append('恢复材料缺少实际passed演练记录')
        for key, expected in [('environment_id', environment.get('vm_id')), ('snapshot_id', environment.get('snapshot_id')),
                              ('snapshot_storage_sha256', environment.get('snapshot_storage_sha256')), ('operator', people.get('test_operator'))]:
            if restore.get(key) != expected:
                issues.append(f'恢复材料{key}与当前环境不一致')
        timestamp(restore.get('performed_at_utc'), '恢复材料performed_at_utc', issues)
    authorization, _ = read_evidence(request.get('signatures_and_authorization_evidence'), base, 'execution_authorization', issues, files)
    if authorization is not None:
        if (authorization.get('schema_version') != 2 or authorization.get('kind') != 'cloudleaf-windows-soak-execution-authorization'
                or authorization.get('status') != 'approved'):
            issues.append('执行授权材料需schema2及实际批准，旧授权不得自动升级')
        if not sha256_text(authorization.get('execution_identity_sha256')):
            issues.append('执行授权缺少本次完整execution_identity_sha256')
        for key, expected in [('environment_id', environment.get('vm_id')), ('snapshot_id', environment.get('snapshot_id')),
                              ('operator', people.get('test_operator')), ('authorized_by', people.get('environment_owner'))]:
            if authorization.get(key) != expected:
                issues.append(f'执行授权{key}与当前环境不一致')
        timestamp(authorization.get('authorized_at_utc'), '执行授权authorized_at_utc', issues)
        profiles = authorization.get('allowed_profiles')
        if not isinstance(profiles, list) or not profiles or any(name not in ['8h', '24h'] for name in profiles):
            issues.append('执行授权allowed_profiles需要明确8h/24h范围')
    materials_consistent = not issues
    binding_checked = identity is not None and authorization is not None
    if identity is not None:
        if authorization is not None:
            if authorization.get('execution_identity_sha256') != execution_identity_sha256(identity):
                issues.append('执行授权与本次完整源码/数据/工具/二进制/配置/系统身份不一致')
            profiles = authorization.get('allowed_profiles')
            if isinstance(profiles, list) and identity['configuration']['profile'] not in profiles:
                issues.append('本次长期档不在执行授权范围')
        for field, actual in [('windows_version_and_build', identity['environment']['os']),
                              ('architecture', identity['environment']['architecture'])]:
            if environment.get(field) != actual:
                issues.append(f'environment.{field}与实际运行身份不符')
    else:
        issues.append('未提供本次实际运行identity，材料一致不能授予执行准入')
    result = admission_report(issues, files)
    result.update(materials_consistent=materials_consistent, execution_identity_binding_checked=binding_checked,
                  ready=not issues and binding_checked)
    result['environment_id'] = environment.get('vm_id')
    result['snapshot_id'] = environment.get('snapshot_id')
    result['operator'] = people.get('test_operator')
    return result


def check_bound_request(reference, base, identity):
    issues, files = [], []
    request, path = read_evidence(reference, base, 'environment_request', issues, files)
    if request is None:
        return admission_report(issues, files)
    result = check_request(request, path.parent, identity)
    result['checked_files'] = files + result['checked_files']
    return result


if __name__ == '__main__':
    raise SystemExit(main())

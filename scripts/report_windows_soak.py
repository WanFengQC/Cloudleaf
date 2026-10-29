"""只读重放长期诊断原件，逐段描述资源趋势，不授予平台或长期验收。"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re

import stress_windows_faults as stress

RESOURCES = ('handles', 'threads', 'working_set_bytes', 'private_commit_bytes')
CYCLE_VALUES = ('hosts', 'batches', 'fixed_bytes', 'text_bytes', 'host_rc_strong_count',
                'module_leases', 'module_locks', 'own_iunknown_addref_return', 'own_iunknown_release_return')
FILES = {'stdout.txt', 'samples.jsonl', 'events.jsonl', 'checkpoints.jsonl', 'checkpoint.json'}


def report_run(folder):
    folder = folder.resolve(strict=True)
    manifest_raw = (folder / 'manifest.json').read_bytes()
    manifest = json.loads(manifest_raw)
    if manifest.get('schema_version') != 2:
        raise ValueError('仅支持既有runner manifest schema2')
    identity = manifest['identity']
    config = identity['configuration']
    expected = stress.configuration(config['profile'], config['seconds'], config['cycle_ms'],
                                    config['checkpoint_seconds'], config['sample_seconds'])
    if config != expected:
        raise ValueError('manifest配置与固定profile约定不符')
    segments = sorted(path for path in folder.glob('segment-*') if path.is_dir())
    if not segments or any(not re.fullmatch(r'segment-\d{4}', path.name) for path in segments):
        raise ValueError('缺少有效segment目录')
    return {'schema_version': 1, 'experiment': 'cloudleaf-windows-soak-resource-report-v1',
            'run_directory': str(folder), 'run_id': manifest['run_id'], 'identity': identity,
            'manifest_sha256': digest(manifest_raw),
            'reporter_sha256': digest(Path(__file__).read_bytes()),
            'trace_auditor_sha256': digest(Path(stress.__file__).read_bytes()),
            'segments': [report_segment(path, identity) for path in segments],
            'segments_combined_to_claim_duration': False,
            'long_term_production_reliability_accepted': False, 'real_application_acceptance': False,
            'system_registration_executed': False,
            'limitations': ['仅重放既有原件，不运行测试、采样、注册或操作宿主。',
                            '资源时基为runner相对该段启动的perf_counter秒，包含启动和故障准备，不是纯稳态。',
                            'OLS每小时斜率仅描述当前样本；短段不稳，未冻结阈值不能判定泄漏或合格。',
                            'native持续时长来自自有工程host声明，不是独立外部时钟或真实TSF结果。',
                            '不同segment独立呈现，不拼接；工具不证明隔离环境、批准或人工验收真实性。']}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def raw_file(path, expected=None):
    physical = path.read_bytes()
    raw, envelope = physical, None
    resolution = 'physical'
    if (expected is not None and digest(raw) != expected
            and path.name in {'checkpoints.jsonl', 'events.jsonl', 'samples.jsonl'}):
        candidate = physical.replace(b'\r\n', b'\n')
        if digest(candidate) == expected:
            raw = candidate
            resolution = 'exact-sha-crlf-to-lf'
    if expected is not None and digest(raw) != expected and path.name == 'stdout.txt':
        candidate = path.with_suffix('.raw.json')
        if candidate.exists():
            value = json.loads(candidate.read_bytes())
            raw = value['raw_text'].encode('utf-8')
            if digest(raw) != value['original_sha256'] or len(raw) != value['original_bytes']:
                raise ValueError('stdout原文封装损坏')
            envelope = {'path': str(candidate), 'sha256': digest(candidate.read_bytes())}
            resolution = 'explicit-original-envelope'
    return raw, {'path': str(path), 'physical_bytes': len(physical), 'physical_sha256': digest(physical),
                 'resolved_original_bytes': len(raw), 'resolved_original_sha256': digest(raw),
                 'original_resolution': resolution, 'original_envelope': envelope}


def read_rows(raw, name, issues):
    rows = []
    for number, line in enumerate(raw.decode('utf-8').splitlines(), 1):
        try:
            value = json.loads(line)
            if not isinstance(value, dict):
                raise ValueError('记录必须为对象')
            rows.append(value)
        except (ValueError, TypeError) as error:
            issues.append(f'{name}:{number}:{error}')
    return rows


def metric_values(values):
    return {'count': len(values), 'first': values[0] if values else None, 'last': values[-1] if values else None,
            'min': min(values) if values else None, 'max': max(values) if values else None}


def resource_trend(rows):
    if not rows:
        return None
    times = [row['elapsed_seconds'] for row in rows]
    intervals = [right - left for left, right in zip(times, times[1:])]
    ordered = sorted(intervals)
    mean_x = math.fsum(times) / len(times)
    denominator = math.fsum((time - mean_x) ** 2 for time in times)
    resources = {}
    for name in RESOURCES:
        values = [row[name] for row in rows]
        mean_y = math.fsum(values) / len(values)
        slope = (math.fsum((time - mean_x) * (value - mean_y) for time, value in zip(times, values))
                 / denominator * 3600 if denominator else None)
        resources[name] = {**metric_values(values), 'delta_last_minus_first': values[-1] - values[0],
                           'ols_units_per_hour': slope}
    return {'samples': len(rows), 'first_elapsed_seconds': times[0], 'last_elapsed_seconds': times[-1],
            'sample_span_seconds': times[-1] - times[0],
            'sample_interval_seconds': {'count': len(intervals),
                                        'median': ((ordered[(len(ordered) - 1) // 2] + ordered[len(ordered) // 2]) / 2
                                                   if ordered else None),
                                        'max': max(intervals) if intervals else None},
            'resources': resources, 'resource_gate_passed': None}


def report_segment(folder, identity):
    issues, files, raw = [], {}, {}
    summary_path = folder / 'summary.json'
    summary = json.loads(summary_path.read_bytes()) if summary_path.exists() else None
    if summary is not None and (summary.get('schema_version') != 2
                                or summary.get('explicit_configuration') != identity['configuration']):
        issues.append('summary版本或配置与manifest不一致')
    expected = summary.get('files_sha256', {}) if summary else {}
    if summary is not None and set(expected) != FILES:
        issues.append('summary未完整登记固定五份原始文件')
    for name in sorted(FILES):
        path = folder / name
        if not path.exists():
            issues.append(f'原件缺失:{name}')
            continue
        try:
            raw[name], files[name] = raw_file(path, expected.get(name))
            if name in expected and digest(raw[name]) != expected[name]:
                issues.append(f'原件SHA不符:{name}')
        except (ValueError, KeyError, TypeError) as error:
            issues.append(f'原件读取失败:{name}:{error}')
    if summary:
        files['summary.json'] = {'path': str(summary_path), 'physical_sha256': digest(summary_path.read_bytes())}
    samples = read_rows(raw.get('samples.jsonl', b''), 'samples', issues)
    checkpoints = read_rows(raw.get('checkpoints.jsonl', b''), 'checkpoints', issues)
    checkpoint = json.loads(raw['checkpoint.json']) if 'checkpoint.json' in raw else None
    last_time = -1
    sample_valid = True
    for number, row in enumerate(samples, 1):
        time = row.get('elapsed_seconds')
        if (type(time) not in {int, float} or not math.isfinite(time) or time < 0 or time <= last_time
                or any(type(row.get(name)) is not int or row[name] < 0 for name in RESOURCES)):
            issues.append(f'samples:{number}:时间或资源值无效')
            sample_valid = False
        else:
            last_time = time
    previous = -1
    for number, row in enumerate(checkpoints, 1):
        time = row.get('elapsed_seconds')
        if (type(time) not in {int, float} or not math.isfinite(time) or time < previous
                or (summary and (row.get('runner') != summary['runner']
                                 or row.get('child') not in [None, summary['child']]))):
            issues.append(f'checkpoints:{number}:时间或进程身份不一致')
        else:
            previous = time
    if not checkpoints or checkpoint != checkpoints[-1]:
        issues.append('最新checkpoint与原始检查点流不一致或缺失')
    if summary and checkpoint and (checkpoint.get('runner') != summary['runner']
                                  or checkpoint.get('child') != summary['child']
                                  or checkpoint.get('exit_code') != summary['exit_code']
                                  or checkpoint.get('elapsed_seconds') != summary['wall_seconds']):
        issues.append('最后checkpoint与summary进程/退出/时基不一致')
    if not samples:
        issues.append('资源采样缺失')
    if summary and samples and sample_valid and any(
            summary.get('sampled_peak', {}).get(name) != max(row[name] for row in samples) for name in RESOURCES):
        issues.append('summary资源峰值与原始采样不一致')
    latency = stress.latency_summary(folder / 'events.jsonl') if 'events.jsonl' in raw else None
    if latency and latency['raw_errors']:
        issues.append('原始按键协议有错误，见latency.raw_errors')
    if summary and latency and any(summary.get('latency', {}).get(name) != latency[name]
                                   for name in ['events', 'cycles', 'final_records', 'trace_complete']):
        issues.append('summary与原始按键分母不一致')
    observations = [row for row in read_rows(raw.get('events.jsonl', b''), 'events', issues)
                    if row.get('experiment') in {'r6-cycle', 'r5-pinned-availability'}]
    cycle_rows = [row for row in observations if row['experiment'] == 'r6-cycle']
    budget = {}
    for name in CYCLE_VALUES:
        values = [row[name] for row in cycle_rows if name in row]
        if any(type(value) is not int or value < 0 for value in values):
            issues.append(f'cycle观测值无效:{name}')
        else:
            budget[name] = metric_values(values)
    finals = [row for row in observations if row['experiment'] == 'r5-pinned-availability']
    final = finals[-1] if finals else {}
    if summary and summary.get('latest_events', {}).get('r5-pinned-availability', {}) != final:
        issues.append('summary final快照与原始事件不一致')
    native_ns = final.get('steady_elapsed_ns')
    native_seconds = native_ns / 1_000_000_000 if native_ns is not None else None
    config = identity['configuration']
    candidate = bool(summary and not issues and config['long_profile'] and summary['mechanism_passed']
                     and summary['continuous_long_profile_completed'] and latency and latency['trace_complete']
                     and len(samples) >= 2 and last_time >= config['seconds']
                     and native_seconds is not None and native_seconds >= config['seconds']
                     and identity.get('execution_approval_sha256'))
    return {'segment': folder.name, 'files': files, 'summary_present': summary is not None,
            'evidence_integrity_verified': summary is not None and not issues, 'issues': issues,
            'raw_sample_records': len(raw.get('samples.jsonl', b'').splitlines()),
            'checkpoint_records': len(checkpoints),
            'resources': resource_trend(samples) if sample_valid else None,
            'cycle_observations': budget, 'latency': latency,
            'controller_wall_seconds': summary.get('wall_seconds') if summary else None,
            'native_steady_seconds': native_seconds, 'configured_seconds': config['seconds'],
            'exit_code': summary.get('exit_code') if summary else None,
            'interrupted': summary.get('interrupted') if summary else None,
            'recovery': summary.get('recovery') if summary else None,
            'candidate_contiguous_long_record_for_external_review': candidate,
            'long_term_production_reliability_accepted': False, 'real_application_acceptance': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('输出已存在，禁止覆盖历史报告')
    try:
        report = report_run(args.run_dir)
    except (ValueError, TypeError, KeyError, OSError) as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    valid = all(not row['issues'] for row in report['segments'])
    print(json.dumps({'segments': len(report['segments']), 'evidence_valid': valid,
                      'long_term_production_reliability_accepted': False}, ensure_ascii=False))
    return 0 if valid else 2


if __name__ == '__main__':
    raise SystemExit(main())

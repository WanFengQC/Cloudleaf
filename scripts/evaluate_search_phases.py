"""冻结真实词库的可选阶段剖析；保留拒绝/预算/剪枝和全部原始样本。"""
from __future__ import annotations
import argparse
from collections import Counter
from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess

from benchmark import ROOT, environment, source_sha256
from evaluate_real_lexicon import CONFIGURATIONS, CASES, PROTOCOL, validate_evidence, validate_cases, sha256, measure
from analyze_search_round3 import complete, compare_rows, quantiles

PHASES = ('query_total', 'normalize', 'lattice', 'search_validate', 'traversal', 'bucket_sort_dedup',
          'final_sort_dedup', 'context_rerank', 'materialize')
SEMANTICS = ('status', 'error_kind', 'candidates', 'waiting', 'fallback_preserved', 'guaranteed',
             'whole_entry_candidates', 'whole_entry_complete', 'statistics')


def build_host() -> Path:
    result = subprocess.run(['cargo', 'build', '--release', '--locked', '--offline', '-p',
                             'cloudleaf-search', '--example', 'resource_phase_probe', '--message-format=json'],
                            cwd=ROOT, capture_output=True, text=True, encoding='utf-8')
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
    paths = [Path(row['executable']) for row in artifacts if row.get('reason') == 'compiler-artifact'
             and row.get('target', {}).get('name') == 'resource_phase_probe' and row.get('executable')]
    if len(paths) != 1:
        raise ValueError('Cargo没有返回唯一实际阶段宿主')
    return paths[0]


def validate_sample(sample: dict, rejected: bool, index: int) -> None:
    if (sample.get('equal') is not True or sample.get('plain_first') != (index % 2 == 0)
            or any(type(sample.get(key)) is not int or sample[key] < 0 for key in ('plain_ns', 'profile_outer_ns'))):
        raise ValueError('样本缺失或观测改变行为/配对顺序')
    phases = sample.get('profile', {}).get('phases', {})
    if set(phases) != set(PHASES):
        raise ValueError('阶段字段缺失或重复')
    calls = 0
    for name, value in phases.items():
        if value is None:
            continue
        if (set(value) != {'ns', 'calls'} or type(value['ns']) is not int or value['ns'] < 0
                or type(value['calls']) is not int or value['calls'] < 1):
            raise ValueError('阶段时间/调用数错误')
        calls += value['calls']
    if sample['profile'].get('clock_reads') != calls * 2:
        raise ValueError('实际读钟数与阶段调用数不一致')
    if any(phases[name] is None or phases[name]['calls'] != 1 for name in ('query_total', 'normalize')):
        raise ValueError('拒绝/成功均须保留归一化和总时')
    if rejected:
        if any(phases[name] is not None for name in PHASES[2:]):
            raise ValueError('当前非法输入未进入后续阶段，不能伪造计时零值')
    elif any(phases[name] is None or phases[name]['calls'] != 1 for name in
             ('lattice', 'search_validate', 'traversal', 'final_sort_dedup', 'materialize')):
        raise ValueError('成功或预算结果缺少实际已进入阶段')


def validate_run(run: dict, cases: list[dict], iterations: int, warmup: int) -> dict:
    if run.get('language_accuracy_valid') is not False or run.get('iterations_per_case') != iterations or run.get('warmup_per_case') != warmup:
        raise ValueError('实验语言声明或次数漂移')
    rows = run.get('cases', [])
    expected = {row['id']: row for row in cases}
    if len(rows) != len(expected) or {row['id'] for row in rows} != set(expected):
        raise ValueError('丢失/重复案例，禁止删除失败分母')
    counts = Counter()
    all_samples = []
    for row in rows:
        if any(row.get(key) != expected[row['id']][key] for key in ('family', 'input', 'kind')):
            raise ValueError('案例输入/家族被改变')
        samples = row.get('samples', [])
        if len(samples) != iterations:
            raise ValueError('原始阶段样本数量不符')
        result = row['result']
        status = result.get('status')
        if status not in ('ok', 'rejected'):
            raise ValueError('结果状态缺失')
        for index, sample in enumerate(samples):
            validate_sample(sample, status == 'rejected', index)
        stats = result.get('statistics', {})
        outcome = ('rejected' if status == 'rejected' else 'budget' if stats.get('budget') else
                   'beam_pruned' if stats.get('beam_pruned') else 'complete')
        counts[outcome] += 1
        all_samples.extend(samples)
    phases = {}
    for name in PHASES:
        present = [sample['profile']['phases'][name] for sample in all_samples if sample['profile']['phases'][name] is not None]
        phases[name] = {'attempted_samples': len(present), 'not_entered_samples': len(all_samples) - len(present),
                        'nanos': quantiles([row['ns'] for row in present]),
                        'calls': quantiles([row['calls'] for row in present])}
    return {'cases': len(rows), 'outcomes': dict(counts), 'samples': len(all_samples), 'phases': phases,
            'plain_ns': quantiles([row['plain_ns'] for row in all_samples]),
            'profile_outer_ns': quantiles([row['profile_outer_ns'] for row in all_samples]),
            'clock_reads': quantiles([row['profile']['clock_reads'] for row in all_samples]),
            'stage_sums_valid': False, 'exclusive_cpu_cost': None}


def validate_historical(run: dict, old: dict) -> list[dict]:
    indexed = {row['id']: row for row in old['cases']}
    comparisons = []
    for row in run['cases']:
        previous = indexed[row['id']]
        result = row['result']
        fields = {key: result.get(key) == previous.get(key) for key in SEMANTICS}
        comparisons.append({'id': row['id'], 'all_observed_fields_equal': all(fields.values()), 'fields': fields,
                            'complete_reference': complete(previous),
                            'full_algorithm_equivalence': all(fields.values()) if complete(previous) and complete(result) else None})
    return comparisons


def validate_historical_identity(old: dict, runtime_sha: str, cases_sha: str, protocol_sha: str) -> None:
    if (old.get('evidence', {}).get('runtime', {}).get('sha256') != runtime_sha
            or old.get('cases_sha256') != cases_sha or old.get('protocol_sha256') != protocol_sha):
        raise ValueError('历史数据/探针/协议身份不同，拒绝配对')
    if set(old.get('runs', {})) != set(CONFIGURATIONS):
        raise ValueError('历史配置集合不一致')
    for label, (method, width, edges, expansions, resident) in CONFIGURATIONS.items():
        expected = {'method': method, 'beam_width': width, 'max_edges': edges, 'max_expansions': expansions,
                    'max_resident_paths': resident, 'max_input_bytes': 512, 'max_letters': 256,
                    'output_k': 5, 'ranker': 'Structural'}
        if old['runs'][label].get('configuration') != expected:
            raise ValueError('历史完整搜索配置不一致：' + label)


def run() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--historical', type=Path, required=True)
    parser.add_argument('--iterations', type=int, default=30)
    parser.add_argument('--warmup', type=int, default=5)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    if args.output_dir.exists():
        parser.error('输出目录已存在，禁止覆盖旧证据')
    args.output_dir.mkdir(parents=True)
    runtime, evidence, counts = validate_evidence(args.evidence)
    cases = validate_cases()
    old = json.loads(args.historical.read_text(encoding='utf-8'))
    validate_historical_identity(old, sha256(runtime), sha256(CASES), sha256(PROTOCOL))
    before = source_sha256()
    binary = build_host()
    runs, summaries, historical, failures = {}, {}, {}, []
    for label, (method, width, edges, expansions, resident) in CONFIGURATIONS.items():
        command = [str(binary), str(runtime), str(CASES), method, str(width), str(args.iterations),
                   str(args.warmup), str(edges), str(expansions), str(resident), '5']
        try:
            result, peak, memory_method = measure(command, stdout_path=args.output_dir / f'{label}-stdout.json',
                                                  stderr_path=args.output_dir / f'{label}-stderr.txt')
        except (RuntimeError, ValueError) as error:
            (args.output_dir / 'failure-index.json').write_text(json.dumps({'failed_configuration': label,
                    'command': command, 'reason': str(error), 'completed_configurations': list(runs),
                    'source_sha256': before, 'binary_sha256': sha256(binary),
                    'stdout_path': f'{label}-stdout.json', 'stderr_path': f'{label}-stderr.txt'}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
            raise
        result.update(command=command, peak_process_memory_bytes=peak, memory_method=memory_method,
                      configuration={'method': method, 'beam_width': width, 'max_edges': edges,
                                     'max_expansions': expansions, 'max_resident_paths': resident})
        # 每配置先保存原件；分析失败不能丢弃已经取得的实际输出。
        (args.output_dir / f'{label}.json').write_text(json.dumps(result, ensure_ascii=False, separators=(',', ':')) + '\n', encoding='utf-8', newline='\n')
        runs[label] = result
        summaries[label] = validate_run(result, cases, args.iterations, args.warmup)
        historical[label] = validate_historical(result, old['runs'][label])
        failures.extend(f'{label}:{row["id"]}' for row in historical[label] if not row['all_observed_fields_equal'])
        print(json.dumps({'configuration': label, 'outcomes': summaries[label]['outcomes'], 'samples': summaries[label]['samples']}, ensure_ascii=False), flush=True)
    if source_sha256() != before:
        raise RuntimeError('测量期间源码改变')
    validate_evidence(args.evidence)
    report = {'schema_version': 1, 'experiment': 'cloudleaf-search-phases-round5', 'language_accuracy_valid': False,
              'top1': None, 'top5': None, 'mrr': None, 'source_sha256': before, 'source_fingerprint_schema': 'cloudleaf-source-v2',
              'git_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'binary_sha256': sha256(binary), 'runtime_sha256': sha256(runtime), 'cases_sha256': sha256(CASES),
              'protocol_sha256': sha256(ROOT / 'docs/cl008-round5-profiling-design.md'),
              'original_protocol_sha256': sha256(PROTOCOL), 'historical_report_sha256': sha256(args.historical),
              'evidence_sha256': sha256(args.evidence), 'lexicon_counts': counts, 'environment': environment(),
              'measured_at_utc': datetime.now(timezone.utc).isoformat(), 'summaries': summaries,
              'historical_behavior_comparisons': historical, 'behavior_failures': failures,
              'raw_configuration_reports': {label: {'path': f'{label}.json', 'sha256': sha256(args.output_dir / f'{label}.json')} for label in runs},
              'limitations': ['bucket_sort_dedup嵌套在traversal，query_total包含所有阶段及读钟开销；不能相加或相减伪造独立CPU成本。',
                              '与未观测旧入口逐样本全字段比较，profile耗时不代表产品或TSF输入延迟；峰值为整个宿主进程。',
                              '实际有词库而没有独立人工gold，所有预算/非法/空结果保留，语言质量指标null。']}
    (args.output_dir / 'summary.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    if failures:
        raise SystemExit(1)


if __name__ == '__main__':
    run()

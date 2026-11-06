"""交错执行同一171事件；比较完整输出，正式语言指标始终null。"""
from pathlib import Path
import hashlib
import json
import math
import subprocess
import sys
root = Path(__file__).resolve().parents[1]
out = root / '.local/round17-final-paired-traces'
out.mkdir(exist_ok=False)
variants = ['originalcharacter', 'character', 'originalwitten', 'witten', 'originallexical', 'lexical']
rows = []
all_times = {variant: [] for variant in variants}

def stable_output(lines):
    return [line for line in lines if line.startswith(('FRAME\t', 'CANDIDATE\t', 'COMMIT\t', 'SEARCH\t','PATH\t'))]

def percentile(times, p):
    return sorted(times)[math.ceil(len(times) * p / 100) - 1]

for iteration in range(1, 6):
    order = variants[iteration % len(variants):] + variants[:iteration % len(variants)]
    records = {}
    for variant in order:
        label = f'round17-final-{variant}-{iteration}'
        result = subprocess.run([sys.executable, '-X', 'utf8', '.local/trace17.py', label, variant], cwd=root, capture_output=True)
        (out / f'{label}.launcher.stdout').write_bytes(result.stdout)
        (out / f'{label}.launcher.stderr').write_bytes(result.stderr)
        assert result.returncode == 0
        record = json.loads((root / '.local' / label / 'result.json').read_text(encoding='utf-8'))
        lines = (root / '.local' / label / 'stdout.tsv').read_text(encoding='utf-8').splitlines()
        times = [int(line.split('\t')[3]) for line in lines if line.startswith('EVENT\t')]
        assert len(times) == 171
        all_times[variant].extend(times)
        records[variant] = lines
        rows.append({'iteration': iteration, 'variant': variant, 'label': label, 'result': record})
        print(json.dumps({'iteration': iteration, 'variant': variant, 'latency_ns': record['event_latency_ns']}), flush=True)
    for kind in ['character','witten','lexical']:
        assert stable_output(records['original'+kind]) == stable_output(records[kind]), kind+' complete output and statistics changed'

def case_details(lines):
    cases = {}
    for line in lines:
        fields = line.split('\t')
        if fields[0] == 'FRAME':
            cases[fields[1]] = {'raw': json.loads(fields[4]), 'candidates': [], 'paths': [], 'search': None}
        elif fields[0] == 'CANDIDATE':
            cases[fields[1]]['candidates'].append({'index': int(fields[2]), 'text': json.loads(fields[3]),
                'id': fields[4], 'frequency': fields[5], 'match_kind': fields[6]})
        elif fields[0] == 'PATH':
            cases[fields[1]]['paths'].append({'id': fields[2], 'score': int(fields[3]),
                'features': fields[4], 'edges': fields[5]})
        elif fields[0] == 'SEARCH':
            cases[fields[1]]['search'] = fields[2:]
    return cases

comparisons = {}
for variant in variants:
    row = next(row for row in rows if row['variant'] == variant)
    comparisons[variant] = case_details((root / '.local' / row['label'] / 'stdout.tsv').read_text(encoding='utf-8').splitlines())
summary = {'implementation_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip(),
    'original_implementation': '521472db18ecd7926f6e5b58b0afd7b5da674a07',
    'old_lexical_output_and_statistics_identical': True,
    'character_model_sha256': hashlib.sha256((root/'.local/round16-language/character.tsv').read_bytes()).hexdigest(),
    'model_sha256': hashlib.sha256((root / '.local/round15-language/signal.tsv').read_bytes()).hexdigest(),
    'events_sha256': hashlib.sha256((root / 'tests/fixtures/input-round14-events.tsv').read_bytes()).hexdigest(),
    'same_events_input_config_and_resource_budgets': True, 'optimized_structural_output_and_statistics_identical': True,
    'timing_scope': 'InputEngine.handle including frame clone, excluding stdout/platform/disk; mixed engineering events, five fresh processes per variant, OS cache not cleared; sampler50ms',
    'warmup': 'No discarded warmup: all five runs and every event retained.',
    'percentiles_ns': {variant: {'n': len(times), **{f'p{p}': percentile(times, p) for p in [50, 95, 99]}, 'max': max(times)}
        for variant, times in all_times.items()},
    'runs': rows, 'cases': comparisons, 'formal_language_metrics': {'top1': None, 'top5': None, 'mrr': None},
    'TSF': False, 'incremental_enabled': False}
(out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({'percentiles_ns': summary['percentiles_ns'], 'structural_equal': True}, ensure_ascii=False))

"""新schema2评分诊断，固定原输入/预算并全字段核对R6最终结果。"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / '.local/round7-phases-2a2bd9a'
OUT.mkdir(exist_ok=False)
sys.path.insert(0, str(ROOT/'scripts'))
from benchmark import source_sha256
command = [sys.executable, '-X', 'utf8', 'scripts/evaluate_search_phases.py',
           '--evidence', '.local/real-lexicon/cccedict-round4-evidence.json',
           '--historical', 'docs/reports/cl008-round4/cccedict-full.json', '--iterations', '10', '--warmup', '3',
           '--output-dir', str(OUT/'matrix')]
env = os.environ.copy()
env['PATH'] = str(Path.home()/'.cargo/bin') + os.pathsep + env['PATH']
env['PYTHONUTF8'] = '1'
env['PYTHONIOENCODING'] = 'utf-8'
before = source_sha256()
head = subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip()
assert head == '2a2bd9a7ea9df73b647ccdba609d9aae8f0ad935'
start = time.perf_counter()
result = subprocess.run(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
(OUT/'command.txt').write_bytes(result.stdout)
identity = {'implementation_commit':head, 'source_sha256':before, 'source_fingerprint_schema':'cloudleaf-source-v2',
            'command':command, 'returncode':result.returncode, 'wall_seconds':time.perf_counter()-start,
            'log_sha256':hashlib.sha256(result.stdout).hexdigest(),
            'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
(OUT/'identity.json').write_text(json.dumps(identity,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
assert result.returncode == 0, result.stdout.decode('utf-8')
comparisons, cases, failures = [], [], []
for path in sorted((OUT/'matrix').glob('*-stdout.json')):
    label = path.name.removesuffix('-stdout.json')
    observed = json.loads(path.read_text(encoding='utf-8'))
    oldpath = ROOT/'docs/reports/cl008-round6/matrix'/f'{label}-after.json'
    old = json.loads(oldpath.read_text(encoding='utf-8'))
    oldrows = {row['id']:row for row in old['cases']}
    for row in observed['cases']:
        equal = row['result'] == oldrows[row['id']]['result'] and row['result_debug'] == oldrows[row['id']]['result_debug']
        comparisons.append({'configuration':label,'id':row['id'],'all_result_fields_and_debug_equal':equal})
        if not equal:
            failures.append({'configuration':label,'id':row['id'],'current':row,'old':oldrows[row['id']]})
        from analyze_search_round3 import quantiles
        outcome = row['result']
        stats = outcome.get('statistics', {})
        cases.append({'configuration':label,'id':row['id'],'status':outcome['status'],
                      'error_kind':outcome.get('error_kind'),'candidate_count':len(outcome.get('candidates', [])),
                      'statistics':stats,'raw_result':outcome,'result_debug':row['result_debug'],
                      'plain_ns':quantiles([sample['plain_ns'] for sample in row['samples']]),
                      'phase_ns':{name:quantiles([sample['profile']['phases'][name]['ns'] for sample in row['samples']
                                  if sample['profile']['phases'][name] is not None])
                                  for name in row['samples'][0]['profile']['phases']}})
assert len(comparisons) == 288
assert source_sha256() == before
summary = {'identity':identity,'r6_compared_implementation':'9673c29151380c146ae6ecb16055072cda0f0f00',
           'cross_version_comparisons':comparisons,'per_case_summaries':cases,'failures':failures,
           'samples_per_case':10,'paired_samples':2880,'warmup_per_case':3,'language_accuracy_valid':False,
           'top1':None,'top5':None,'mrr':None,'single_algorithm_memory_measured':False,
           'limitations':['新schema2观测，n=10的P99不证明稳定尾部。不得与R6 n=30做速度改善/资源门禁比较。',
                         'Scoring和桶均嵌套于traversal，ContextRerank仍独立；不相加/相减推导独占CPU。',
                         '进程峰非单算法内存，当前原型真实TSF及正式语言质量均未验收。']}
(OUT/'comparison.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'returncode':result.returncode,'pairs':2880,'compared_cases':288,'result_differences':len(failures),'wall_seconds':identity['wall_seconds']}))
raise SystemExit(bool(failures))

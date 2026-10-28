"""冻结同源宿主的旧新算法，保存完整结果、阶段、预算及全部失败分母。"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import evaluate_search_phases as phases
from evaluate_real_lexicon import CONFIGURATIONS, CASES, PROTOCOL, validate_evidence, validate_cases, sha256, measure
from benchmark import environment, source_sha256

OUT = ROOT / '.local/round6-matrix'
OUT.mkdir(exist_ok=False)
BASELINE = ROOT / '.local/round6-before/identity.json'
baseline = json.loads(BASELINE.read_text(encoding='utf-8'))
old = Path(baseline['binary'])
assert sha256(old) == baseline['binary_sha256']
assert baseline['head'] == 'f1696fa125d27507be0d328f04717b9760163417'
evidence_path = ROOT / '.local/real-lexicon/cccedict-round4-evidence.json'
runtime, evidence, counts = validate_evidence(evidence_path)
cases = validate_cases()
historical_path = ROOT / 'docs/reports/cl008-round4/cccedict-full.json'
historical = json.loads(historical_path.read_text(encoding='utf-8'))
phases.validate_historical_identity(historical, sha256(runtime), sha256(CASES), sha256(PROTOCOL))
source_before = source_sha256()
new = phases.build_host()
identity = {'implementation_commit': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip(),
            'source_sha256': source_before, 'source_fingerprint_schema': 'cloudleaf-source-v2',
            'current_binary': str(new), 'current_binary_sha256': sha256(new), 'baseline': baseline,
            'runtime_sha256': sha256(runtime), 'cases_sha256': sha256(CASES),
            'protocol_sha256': sha256(PROTOCOL), 'phase_host_source_sha256': sha256(ROOT / 'crates/cloudleaf-search/examples/resource_phase_probe.rs'),
            'tool_sha256': sha256(Path(__file__)), 'environment': environment(), 'lexicon_counts': counts}
(OUT / 'identity.json').write_text(json.dumps(identity, ensure_ascii=False, indent=2)+'\n',encoding='utf-8',newline='\n')
results, failures = {}, []
for number, (label, (method,width,edges,expansions,resident)) in enumerate(CONFIGURATIONS.items()):
    pair = {}
    # 每配置交换旧新进程次序；仍不是受控实验室或逐查询交错版本运行。
    versions = [('before',old),('after',new)] if number % 2 == 0 else [('after',new),('before',old)]
    for version, binary in versions:
        command = [str(binary),str(runtime),str(CASES),method,str(width),'30','5',str(edges),str(expansions),str(resident),'5']
        started = time.perf_counter()
        try:
            value, peak, memory_method = measure(command, stdout_path=OUT/f'{label}-{version}-stdout.json',
                                                  stderr_path=OUT/f'{label}-{version}-stderr.txt')
        except Exception as error:
            (OUT / 'failure-index.json').write_text(json.dumps({'failed_configuration':label,'version':version,
                'command':command,'error':str(error),'completed_configurations':list(results)},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
            raise
        value.update(command=command, wall_seconds=time.perf_counter()-started,
                     peak_process_memory_bytes=peak, memory_method=memory_method)
        path = OUT / f'{label}-{version}.json'
        path.write_text(json.dumps(value,ensure_ascii=False,separators=(',',':'))+'\n',encoding='utf-8',newline='\n')
        summary = phases.validate_run(value,cases,30,5)
        legacy = phases.validate_historical(value,historical['runs'][label])
        assert all(row['all_observed_fields_equal'] for row in legacy), (label,version,'R4 fields drift')
        pair[version] = {'value':value,'summary':summary,'legacy':legacy,'path':path.name,'sha256':sha256(path)}
        print(json.dumps({'configuration':label,'version':version,'outcomes':summary['outcomes'],'samples':summary['samples']},ensure_ascii=False),flush=True)
    left,right = pair['before']['value']['cases'],pair['after']['value']['cases']
    comparisons=[]
    assert len(left)==len(right)==24
    for a,b in zip(left,right):
        equal = a['id']==b['id'] and a['result_debug']==b['result_debug'] and a['result']==b['result']
        comparisons.append({'id':a['id'],'full_debug_and_reported_results_equal':equal})
        if not equal:
            failures.append({'configuration':label,'id':a['id'],'before':a['result_debug'],'after':b['result_debug']})
    results[label] = {'before':{key:value for key,value in pair['before'].items() if key!='value'},
                      'after':{key:value for key,value in pair['after'].items() if key!='value'},
                      'process_order':[version for version,_ in versions], 'full_result_comparisons':comparisons}
    (OUT/'partial-summary.json').write_text(json.dumps({'identity':identity,'runs':results,'failures':failures},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
assert source_sha256()==source_before and sha256(old)==baseline['binary_sha256']
validate_evidence(evidence_path)
report={'identity':identity,'runs':results,'failures':failures,'language_accuracy_valid':False,
        'top1':None,'top5':None,'mrr':None,'paired_samples_per_version':8640,'full_cross_version_cases':288,
        'limits':['阶段嵌套不可相加，进程峰包含词库/双图/双结果/JSON；无allocation/live/RSS单算法测量。',
                  '相同预算/输入/词库、同宿主源码，各配置交换新旧进程顺序；背景负载未控制，性能只是本机诊断。',
                  '没有独立人工gold；同源版本完整字段相等不等于中文准确率。']}
(OUT/'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
raise SystemExit(1 if failures else 0)

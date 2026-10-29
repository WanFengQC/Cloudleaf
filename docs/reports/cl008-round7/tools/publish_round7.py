"""保存Round7实际原文/发布件双SHA，不覆盖任何既有报告。"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT/'docs/reports/cl008-round7'
OUT.mkdir(exist_ok=False)
BASE = '417dad662bfebedf3238f909476240427cc9100b'
IMPLEMENTATION = '2a2bd9a7ea9df73b647ccdba609d9aae8f0ad935'
sys.path.insert(0,str(ROOT/'scripts'))
from benchmark import source_sha256


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def save(path,value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')


publication, logs, originals = {}, [], []


def copy(source,relative):
    raw = source.read_bytes()
    text = raw.decode('utf-8')
    if source.suffix in {'.txt','.log'}:
        logs.append({'path':relative,'original_path':str(source),'sha256':sha(raw),'bytes':len(raw),'utf8':text})
        display = '\n'.join(line.rstrip() for line in text.splitlines()).rstrip()
        if display:
            display += '\n'
    else:
        display = text.replace('\r\n','\n')
        if display and not display.endswith('\n'):
            display += '\n'
        if display.encode('utf-8') != raw:
            originals.append({'path':relative,'original_path':str(source),'sha256':sha(raw),'bytes':len(raw),'utf8':text})
    target = OUT/relative
    target.parent.mkdir(parents=True,exist_ok=True)
    target.write_text(display,encoding='utf-8',newline='\n')
    publication[relative] = {'original_path':str(source),'original_sha256':sha(raw),'original_bytes':len(raw),
                             'publication_sha256':sha(target.read_bytes())}


for path in sorted((ROOT/'.local/round7-checks-2a2bd9a').rglob('*')):
    if path.is_file():
        copy(path,'checks/'+path.relative_to(ROOT/'.local/round7-checks-2a2bd9a').as_posix())
for path in sorted((ROOT/'.local/round7-red-complete').rglob('*')):
    if path.is_file():
        copy(path,'red/'+path.name)
for name in ['identity.json','comparison.json','command.txt']:
    copy(ROOT/'.local/round7-phases-2a2bd9a'/name,'phases/'+name)
for path in sorted((ROOT/'.local/round7-phases-2a2bd9a/matrix').iterdir()):
    # 原stdout包含全部样本；不再复制只追加command/进程峰的同一份大JSON。
    if path.name.endswith(('-stdout.json','-stderr.txt')) or path.name == 'summary.json':
        copy(path,'phases/matrix/'+path.name)
for name in ['LICENSE.txt','NOTICE.txt','source-lock.json','manifest.json','quality.json']:
    copy(ROOT/'.local/real-lexicon/cccedict-round4-2401b16'/name,'cccedict-source/'+name)
copy(ROOT/'.local/real-lexicon/cccedict-round4-evidence.json','cccedict-source/evidence.json')
for name in ['run_round7_checks.py','run_round7_phases.py','run_round7_red.py','audit_round7_handoff_compat.py','publish_round7.py']:
    copy(ROOT/'.local'/name,'tools/'+name)
copy(ROOT/'.local/round7-handoff-compat.json','handoff-compat.json')
save(OUT/'harness-preparation.json',{'scope':'Local evidence harness preparation, not product failure',
     'operation':'The first red runner completed Python failures, then failed launching cargo by bare name on Windows',
     'exit_code':1,'tool_error_excerpt':'FileNotFoundError: [WinError 2] 系统找不到指定的文件。',
     'historical_helper_bytes_archived':False,
     'resolution':'Resolved actual cargo executable with shutil.which; complete red replay at the same ba17cbf implementation is in red/. Original partial Python logs remain in .local/round7-red.'})
save(OUT/'original-logs.json',{'encoding':'UTF-8; encode utf8 without newline conversion to recover original bytes','files':logs})
save(OUT/'original-artifacts.json',{'encoding':'UTF-8; encode utf8 without newline conversion to recover original bytes','files':originals})
listing = subprocess.check_output(['git','ls-tree','-r',BASE,'docs/reports'],cwd=ROOT).decode().splitlines()
history = [{'path':row.split('\t',1)[1],'git_blob_sha1':row.split()[2]} for row in listing]
changed = subprocess.check_output(['git','diff','--name-only',BASE,'--',*[row['path'] for row in history]],cwd=ROOT).decode().splitlines()
deleted = subprocess.check_output(['git','diff','--diff-filter=D','--name-only',BASE,'--','crates','tests','scripts'],cwd=ROOT).decode().splitlines()
assert not changed and not deleted
save(OUT/'history-preserved.json',{'starting_head':BASE,'unchanged_reports':history,'old_tests_deleted':deleted})
phase = json.loads((OUT/'phases/matrix/summary.json').read_text(encoding='utf-8'))
comparison = json.loads((OUT/'phases/comparison.json').read_text(encoding='utf-8'))
checks = json.loads((OUT/'checks/validation.json').read_text(encoding='utf-8'))
assert phase['git_head'] == checks['implementation_commit'] == IMPLEMENTATION
assert source_sha256() == phase['source_sha256'] == comparison['identity']['source_sha256']
assert not comparison['failures']
assert all(row['returncode'] == row['expected_returncode'] for row in checks['checks'])
rust = sum(map(int,re.findall(r'test result: ok\. (\d+) passed',(OUT/'checks/command-06.txt').read_text(encoding='utf-8'))))
python = int(re.search(r'Ran (\d+) tests',(OUT/'checks/command-10.txt').read_text(encoding='utf-8')).group(1))
index = [row for row in comparison['per_case_summaries'] if row['status'] != 'ok'
         or row['statistics'].get('budget') or row['statistics'].get('beam_pruned')]
save(OUT/'failure-and-budget-index.json',{'count':len(index),'all_failures_retained':True,'cases':index})
core = subprocess.check_output(['git','diff','--name-only',BASE,'--','crates/cloudleaf-input','crates/cloudleaf-types',
          'crates/cloudleaf-candidate','crates/cloudleaf-dictionary','crates/cloudleaf-pinyin','apps'],cwd=ROOT).decode().splitlines()
assert not core
save(OUT/'provenance.json',{'implementation_commit':IMPLEMENTATION,'source_sha256':phase['source_sha256'],
    'source_fingerprint_schema':'cloudleaf-source-v2','phase_schema_version':2,'binary_sha256':phase['binary_sha256'],
    'runtime_sha256':phase['runtime_sha256'],'cases_sha256':phase['cases_sha256'],
    'tool_files_sha256':{name:sha((ROOT/name).read_bytes()) for name in ['scripts/check_language_readiness.py',
        'scripts/check_ime_task_handoff.py','scripts/evaluate_search_phases.py','tests/test_ime_task_handoff.py']},
    'publication':publication,'historical_reports_unchanged':len(history),'old_core_changed':core,
    'rust_tests':rust,'python_tests':python,'checks':len(checks['checks']),
    'raw_log_count':len(logs),'raw_artifact_count':len(originals),
    'language_accuracy_valid':False,'single_algorithm_allocation_live_rss_measured':False})
save(OUT/'checksums.json',{'sha256':{path.relative_to(OUT).as_posix():sha(path.read_bytes())
                                  for path in sorted(OUT.rglob('*')) if path.is_file()}})
print(json.dumps({'published_files':len(list(OUT.rglob('*'))),'checksummed_files':len(json.loads((OUT/'checksums.json').read_text(encoding='utf-8'))['sha256']),
                  'raw_logs':len(logs),'raw_artifacts':len(originals),'history':len(history),'failure_budget_index':len(index),
                  'rust_tests':rust,'python_tests':python}))

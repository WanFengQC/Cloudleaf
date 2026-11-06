"""在固定实现提交上逐命令全回归，不以最后返回码代替前面的结果。"""
from pathlib import Path
import json
import subprocess
import sys
root=Path(__file__).resolve().parents[1]
commands=[
    ['git','diff','--check'],['cargo','fmt','--all','--check'],
    ['cargo','clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings'],
    ['cargo','test','--workspace','--all-targets','--locked','--offline'],
    ['cargo','test','--workspace','--doc','--locked','--offline'],
    ['cargo','build','--workspace','--release','--locked','--offline'],
    ['python','scripts/check_workspace.py'],['python','-m','unittest','discover','-s','tests','-p','test_*.py','-v'],
    ['python','scripts/audit_windows_dependencies.py','--audit-binary','C:/Users/EDY/WebstormProjects/Cloudleaf/.local/audit-tool/bin/cargo-audit.exe','--output','.local/round16-final-dependency-audit.json'],
    ['python','scripts/benchmark.py','--iterations','100','--output','.local/round16-performance.json'],
    ['python','scripts/evaluate_algorithms.py','--output-dir','.local/round16-algorithm-v1'],
    ['cargo','build','-p','cloudleaf-input','--examples','--release','--locked','--offline'],
    ['cargo','build','-p','cloudleaf-search','--examples','--release','--locked','--offline'],
]
rows=[]
for i,command in enumerate(commands,1):
    label=f'final-{i:02}'
    result=subprocess.run([sys.executable,'-X','utf8','.local/run16.py',label,*command],cwd=root,capture_output=True)
    record=json.loads((root/'.local/round16-runs'/label/'result.json').read_text(encoding='utf-8'))
    rows.append({'label':label,**{key:record[key] for key in ['command','returncode','seconds','head','stdout','stderr']}})
    (root/'.local/round16-full-results.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'step':i,'returncode':record['returncode'],'seconds':record['seconds']}),flush=True)
    if result.returncode:print(result.stdout.decode('utf-8',errors='replace')[-4000:]+result.stderr.decode('utf-8',errors='replace')[-1000:],flush=True)
raise SystemExit(any(row['returncode'] for row in rows))

"""在同一真实词库/输入/预算下保存四方法逐阶段配对结果，不生成gold。"""
from pathlib import Path
import hashlib
import json
import subprocess
root=Path(__file__).resolve().parents[1]
out=root/'.local/round14-search-comparison';out.mkdir(exist_ok=False)
exe=root/'.local/build/release/examples/resource_phase_probe.exe'
source=Path('C:/Users/EDY/.codex/tmp/cloudleaf-r14-search-review/cases.tsv')
cases=out/'cases.tsv';cases.write_bytes(source.read_bytes())
receipt=json.loads((root/'.local/round14-lexicon-receipt.json').read_text(encoding='utf-8'))
rows=[]
for method in ['reference','exact','position','beam']:
    command=[str(exe),receipt['runtime']['path'],str(cases),method,'8','3','1','16384','200000','50000','5']
    result=subprocess.run(command,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
    (out/(method+'.json')).write_bytes(result.stdout);(out/(method+'.stderr.txt')).write_bytes(result.stderr)
    rows.append({'method':method,'command':command,'returncode':result.returncode,
                 'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
    assert result.returncode==0,(method,result.stderr)
record={'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),
        'data':receipt['runtime'],'exe_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),
        'cases_sha256':hashlib.sha256(cases.read_bytes()).hexdigest(),'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'commands':rows,'paired_plain_profile':True,'repeats':3,'warmups':1,'formal_language_metrics':None,
        'phase_relation':'traversal includes nested scoring and bucket sort; do not sum all timings',
        'memory_scope':'path/edge work counts only; no per-algorithm byte memory claim'}
(out/'result.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'methods':4,'cases':12,'measured_pairs':144,'all_commands_zero':True,'quality':None}))

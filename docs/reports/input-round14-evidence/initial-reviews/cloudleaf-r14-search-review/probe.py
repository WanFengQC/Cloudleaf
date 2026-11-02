"""只读复用既有CL-008真实探针，输出限定在独立审阅目录。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,subprocess
root=Path('C:/Users/EDY/.codex/worktrees/cl008-search/Cloudleaf')
out=Path(__file__).resolve().parent
cases=out/'cases.tsv'
inputs=['nihao','zhongguo','woaini','jintiantianqihenhao','wozhengzaikaifayikuanzhongwenshurufa','zheshiyigebijiaofuzadewenti','xian',"xi'an",'changan','zhangan','qz','zhongwenshuruf']
cases.write_text('# cloudleaf-search-resource-cases-v1\nid\tfamily\tinput\tkind\n'+''.join(f'r14-{i}\tr14-{i}\t{text}\tengineering\n' for i,text in enumerate(inputs)),encoding='utf-8')
exe=root/'target/release/examples/resource_probe.exe'
runtime=root/'.local/real-lexicon/cccedict-round4-2401b16/runtime.tsv'
identity={'at_utc':datetime.now(timezone.utc).isoformat(),'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),
          'binary_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'data_sha256':hashlib.sha256(runtime.read_bytes()).hexdigest(),'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'cases_sha256':hashlib.sha256(cases.read_bytes()).hexdigest(),'formal_quality_metrics':None}
results=[]
for method in ['reference','exact','position','beam']:
    command=[str(exe),str(runtime),str(cases),method,'8','1','0','16384','200000','50000','5']
    result=subprocess.run(command,cwd=root,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
    (out/(method+'.stdout.json')).write_bytes(result.stdout)
    (out/(method+'.stderr.txt')).write_bytes(result.stderr)
    results.append({'command':command,'returncode':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
    assert result.returncode==0
    value=json.loads(result.stdout)
    print(method,[(row['input'],[c['text'] for c in row['candidates']],row.get('statistics',{}).get('budget'),row.get('max_ns')) for row in value['cases']])
(out/'probe-result.json').write_text(json.dumps({'identity':identity,'runs':results},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')

from pathlib import Path
from datetime import datetime, timezone
import hashlib,json,subprocess,sys
root=Path(__file__).resolve().parents[1]; label=sys.argv[1]
out=root/'.local'/label;out.mkdir(exist_ok=False)
exe=root/'.local/build/release/cloudleaf-ime.exe'
runtime=Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
model=root/'.local/round16-language/character.tsv'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
args=[str(exe),'--interactive-input-host',str(runtime),sha(runtime),str(model),sha(model)]
with (out/'stdout.txt').open('wb') as stdout,(out/'stderr.txt').open('wb') as stderr:
    p=subprocess.Popen(args,cwd=root,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
record={'command':args,'pid':p.pid,'source_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),'binary_sha256':sha(exe),'tool_sha256':sha(Path(__file__)),'start_utc':datetime.now(timezone.utc).isoformat(),'only_owned_window':True,'TSF':False}
(out/'launch.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8');print(json.dumps(record))

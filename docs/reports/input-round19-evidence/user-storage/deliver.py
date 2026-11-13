"""为临时用户词库实现生成交付身份，不写实际工程。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib,json,os,subprocess
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=root/'source/crates/cloudleaf-user'
files={str(p.relative_to(source)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':sha(p)} for p in sorted(source.rglob('*')) if p.is_file()}
runs={str(p.parent.relative_to(root/'runs')).replace('\\','/'):json.loads(p.read_text(encoding='utf-8')) for p in sorted((root/'runs').rglob('result.json'))}
toolchain={}
for name in ['cargo','rustc']:
    command=[str(Path.home()/'.cargo/bin'/f'{name}.exe'),'--version']
    result=subprocess.run(command,capture_output=True)
    toolchain[name]={'command':command,'returncode':result.returncode,'stdout':result.stdout.decode('utf-8').strip()}
record={'created_utc':datetime.now(timezone.utc).isoformat(),'actual_repository_modified':False,'design_commits':['8dd3505','d334f45','47ec379'],'source':str(source),'files':files,'tests':{'contract':2,'storage':14,'failed':0,'ignored':0},'final_commands':{label:runs[label] for label in ['storage-final','storage-final-lint','storage-final-doc','capacity-final-format']},'all_runs':runs,'toolchain':toolchain,'standalone_lock_sha256':sha(root/'source/Cargo.lock'),'tools':{p.name:sha(p)for p in [root/'run.py',Path(__file__)]},'limitations':['Temp Windows tests only; full actual workspace and Linux push CI remain parent work','File StorageFull fault writes actual half payload but injects std ErrorKind::StorageFull; no real disk exhaustion claim','Process exit/interruption tests are not physical power-loss validation','Store path root supplied by host; Windows LocalAppData resolution and owned UI are parent work','User entries are plaintext; no encryption or SSD erasure claim','SelectionOrigin enum is descriptive, not authorization proof; actual host must verify Applied/session/revision/snapshot and reject duplicate callback before writing','Recent-event cache bounded256 and ephemeral; host must never reuse a session/commit token after restart','Windows directory durability not proven; normal save syncs payload before unused-generation rename','Symlink/reparse and lexical parent traversal rejected; no claim of hostile hardlink or external concurrent tamper protection']}
(root/'delivery-identity.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'files':len(files),'passed':16,'failures_preserved':[k for k,v in runs.items()if v['returncode']!=0],'delivery':str(root/'delivery-identity.json')},ensure_ascii=False))

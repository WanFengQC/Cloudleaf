from pathlib import Path
import subprocess
import sys
root=Path(__file__).resolve().parents[1]
for variant in ['character','lexical','covered','word','witten']:
    label='round16-delivery-dev-'+variant
    command=[sys.executable,'-X','utf8','.local/run16.py','delivery-dev-'+variant,sys.executable,'-X','utf8','.local/trace16.py',label,variant,'C:/Users/EDY/.codex/tmp/r16-corpus/word-contrast/development-events.tsv']
    result=subprocess.run(command,cwd=root,capture_output=True)
    print(result.stdout.decode('utf-8',errors='replace')[-800:],flush=True)
    print(result.stderr.decode('utf-8',errors='replace')[-800:],flush=True)
    if result.returncode:raise SystemExit(result.returncode)
for kind in ['addone','witten']:
    model=root/'.local/round16-language'/('character.tsv' if kind=='addone' else 'character-witten.tsv')
    import hashlib
    command=[sys.executable,'-X','utf8','.local/run16.py','delivery-native-host-'+kind,'.local/build/release/cloudleaf-ime.exe','--review-input-host','C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv','03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431',str(model),hashlib.sha256(model.read_bytes()).hexdigest()]
    result=subprocess.run(command,cwd=root,capture_output=True)
    print('native',kind,'returncode',result.returncode,flush=True)
    if result.returncode:raise SystemExit(result.returncode)
s=(root/'.local/summarize_delivery16.py').read_text(encoding='utf-8').replace("f'.local/round16-dev-", "f'.local/round16-delivery-dev-")
(root/'.local/summarize_delivery16.py').write_text(s,encoding='utf-8',newline='\n')

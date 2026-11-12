from pathlib import Path
import hashlib,json,os,subprocess,sys
here=Path(__file__).resolve().parent
out=here/(sys.argv[1] if len(sys.argv)>1 else 'comparison-evidence');out.mkdir(exist_ok=False)
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(here/'comparison-target')
rows=[]
for label,args in [('lock',['C:/Users/EDY/.cargo/bin/cargo.exe','generate-lockfile','--offline']),('run',['C:/Users/EDY/.cargo/bin/cargo.exe','run','--release','--locked','--offline'])]:
    p=subprocess.run(args,cwd=here/'comparison',env=env,capture_output=True)
    (out/f'{label}.stdout').write_bytes(p.stdout);(out/f'{label}.stderr').write_bytes(p.stderr)
    rows.append({'command':args,'returncode':p.returncode,'stdout_sha256':hashlib.sha256(p.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(p.stderr).hexdigest()})
    (out/'commands.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
    print(p.stdout.decode('utf-8',errors='replace')[-2000:]+p.stderr.decode('utf-8',errors='replace')[-2000:])
    if p.returncode:raise SystemExit(p.returncode)

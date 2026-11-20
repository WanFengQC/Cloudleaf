from pathlib import Path
import subprocess,json,hashlib,sys
p=Path(__file__).resolve().parent
out=p/sys.argv[1];out.mkdir(exist_ok=False)
command=[str(p/'source/target/release/resource.exe')]
r=subprocess.run(command,capture_output=True)
(out/'stdout.txt').write_bytes(r.stdout);(out/'stderr.txt').write_bytes(r.stderr)
(out/'result.json').write_text(json.dumps({'command':command,'returncode':r.returncode,'source_sha':hashlib.sha256((p/'source/crates/cloudleaf-search/src/search.rs').read_bytes()).hexdigest()},indent=2),encoding='utf-8')
print(r.returncode);print(r.stdout.decode('utf-8')[-7000:]);print(r.stderr.decode('utf-8'))

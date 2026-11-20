from pathlib import Path
import hashlib,json,shutil
root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).resolve().parent
s=(out/'source');s.mkdir(exist_ok=False)
for name in ['cloudleaf-types','cloudleaf-pinyin','cloudleaf-search']:
    shutil.copytree(root/'crates'/name,s/'crates'/name)
manifest=(root/'docs/reports/input-round19-evidence/search/source/Cargo.toml').read_text(encoding='utf-8')
(s/'Cargo.toml').write_text(manifest,encoding='utf-8')
shutil.copytree(root/'docs/reports/input-round19-evidence/search/source/probe',s/'probe')
shutil.copy(root/'docs/reports/input-round19-evidence/search/source/Cargo.lock',s/'Cargo.lock')
(out/'baseline-source.json').write_text(json.dumps({str(p.relative_to(s)):hashlib.sha256(p.read_bytes()).hexdigest()for p in s.rglob('*')if p.is_file()},indent=2),encoding='utf-8')
print(s)

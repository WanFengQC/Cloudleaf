from pathlib import Path
import shutil
src=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).resolve().parent
for name in ['cloudleaf-types','cloudleaf-pinyin','cloudleaf-search']:
    shutil.copytree(src/'crates'/name,out/'crates'/name,dirs_exist_ok=True)
for name in ['Cargo.lock','rust-toolchain.toml']:shutil.copyfile(src/name,out/name)
s=(src/'Cargo.toml').read_text(encoding='utf-8');a=s.index('members = [');b=s.index(']',a)
s=s[:a]+'members = [\n    "crates/cloudleaf-types",\n    "crates/cloudleaf-pinyin",\n    "crates/cloudleaf-search",\n'+s[b:]
(out/'Cargo.toml').write_text(s,encoding='utf-8',newline='\n')

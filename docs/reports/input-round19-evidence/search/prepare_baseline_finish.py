"""固定R18独立源码构建，不修改实际仓库或覆盖已存日志。"""
from pathlib import Path
import subprocess,shutil,json,hashlib
p=Path(__file__).resolve().parent;root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=p/'baseline-source-finish';out.mkdir(exist_ok=False)
files=subprocess.check_output(['git','ls-tree','-r','--name-only','cf54c84d400b627dfa9b246b3800a82d65f061b8','--','crates/cloudleaf-types','crates/cloudleaf-pinyin','crates/cloudleaf-search'],cwd=root).decode().splitlines()
for name in files:
    dst=out/name;dst.parent.mkdir(parents=True,exist_ok=True)
    dst.write_bytes(subprocess.check_output(['git','show','cf54c84d400b627dfa9b246b3800a82d65f061b8:'+name],cwd=root))
for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:
    if (p/'source'/name).is_file():shutil.copyfile(p/'source'/name,out/name)
shutil.copytree(p/'source/probe',out/'probe')
main=(out/'probe/src/main.rs').read_text(encoding='utf-8')
main=main.replace('scheme: PinyinScheme::AbbreviatedPinyin,','scheme: if args.get(4).is_some_and(|v|v=="full"){PinyinScheme::FullPinyin}else{PinyinScheme::AbbreviatedPinyin},')
(out/'probe/src/main.rs').write_text(main,encoding='utf-8')
(p/'source/probe/src/main.rs').write_text(main,encoding='utf-8')
(p/'baseline-finish-identity.json').write_text(json.dumps({'base':'cf54c84d400b627dfa9b246b3800a82d65f061b8','files':{n:hashlib.sha256((out/n).read_bytes()).hexdigest()for n in files}},indent=2)+'\n',encoding='utf-8')

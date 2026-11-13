"""复制必要的只读基线源码，独立工具目录不写实际工作区。"""
from pathlib import Path
import gzip,hashlib,json,shutil,subprocess
root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).resolve().parent
source=out/'source';source.mkdir(exist_ok=False)
copied={}
for name in ['cloudleaf-types','cloudleaf-pinyin','cloudleaf-search']:
    src=root/'crates'/name;dest=source/'crates'/name;shutil.copytree(src,dest)
    for p in dest.rglob('*'):
        if p.is_file():copied[str(p.relative_to(source))]=hashlib.sha256(p.read_bytes()).hexdigest()
manifest='''[workspace]
members = ["crates/cloudleaf-types", "crates/cloudleaf-pinyin", "crates/cloudleaf-search", "probe"]
resolver = "3"
[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.90.0"
license = "MIT"
publish = false
[workspace.dependencies]
cloudleaf-types = { path = "crates/cloudleaf-types", version = "=0.1.0" }
cloudleaf-pinyin = { path = "crates/cloudleaf-pinyin", version = "=0.1.0" }
cloudleaf-search = { path = "crates/cloudleaf-search", version = "=0.1.0" }
[workspace.lints.rust]
unsafe_code = "forbid"
[workspace.lints.clippy]
all = "warn"
[profile.release]
lto = "thin"
codegen-units = 1
'''
(source/'Cargo.toml').write_text(manifest,encoding='utf-8')
(source/'probe/src').mkdir(parents=True)
(source/'probe/Cargo.toml').write_text('''[package]
name="r19-search-probe"
version="0.1.0"
edition="2024"
[dependencies]
cloudleaf-search.workspace=true
cloudleaf-pinyin.workspace=true
[lints]
workspace=true
''',encoding='utf-8')
summary=json.loads(gzip.decompress((root/'docs/reports/input-round18-evidence/performance/summary.json.gz').read_bytes()))
budget=summary['budget_observations']
(out/'r18-failures.json').write_text(json.dumps({'unique_failure_count':len(budget),'failures':budget,'scope':'R18 each20cases,2character models; repeated3times=90/837 finalprofiles'},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(out/'source-identity.json').write_text(json.dumps({'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),'copied_files':copied,'R18_fixed_case_set_sha256':hashlib.sha256((root/'docs/reports/input-round18-evidence/performance/abbreviated-cases.tsv').read_bytes()).hexdigest()},indent=2)+'\n',encoding='utf-8')
print(json.dumps({'head':'cf54c84','unique_r18_failures':len(budget),'files_copied':len(copied)}))

from pathlib import Path
import shutil,subprocess,os,json,hashlib,sys
here=Path(__file__).resolve().parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
source=here/'source';source.mkdir(exist_ok=True)
for name in ['cloudleaf-pinyin','cloudleaf-types']:
    target=source/'crates'/name
    if not target.exists():shutil.copytree(repo/'crates'/name,target)
(source/'Cargo.toml').write_text('''[workspace]
members=["crates/cloudleaf-pinyin","crates/cloudleaf-types"]
resolver="3"
[workspace.package]
version="0.1.0"
edition="2024"
rust-version="1.90.0"
license="MIT"
publish=false
[workspace.dependencies]
cloudleaf-types={path="crates/cloudleaf-types",version="=0.1.0"}
[workspace.lints.rust]
unsafe_code="forbid"
[workspace.lints.clippy]
all="warn"
''',encoding='utf-8',newline='\n')
shutil.copyfile(repo/'rust-toolchain.toml',source/'rust-toolchain.toml')
lib=subprocess.check_output(['git','show','30b70bc:crates/cloudleaf-pinyin/src/lib.rs'],cwd=repo).decode('utf-8')
lib=lib.replace('use std::cmp::Ordering;', '''mod scheme;
pub use scheme::{PinyinScheme, DoublePinyinLayout, SchemeConfig, InterpretationLimits, CorrectionConfig, FuzzyPair, FuzzyDirection, FuzzyRule, BaseMatch, PhoneticMatch, ChangeKind, PhoneticChange, PhoneticArc, PhoneticHypotheses, InterpretationBudget, InterpretationProfile, interpret, interpret_profiled, XIAOHE_LAYOUT_VERSION};

pub fn legal_syllables() -> &'static [&'static str] { syllables() }

use std::cmp::Ordering;''')
(source/'crates/cloudleaf-pinyin/src/lib.rs').write_text(lib,encoding='utf-8',newline='\n')
for name in ['scheme.rs','xiaohe.rs']:shutil.copyfile(here/name,source/'crates/cloudleaf-pinyin/src'/name)
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(here/'target')
out=here/(sys.argv[1] if len(sys.argv)>1 else 'runs');out.mkdir(exist_ok=False)
commands=[['generate-lockfile','--offline'],['fmt','--all'],['test','--workspace','--all-targets','--locked','--offline'],['clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings']]
rows=[]
for i,args in enumerate(commands):
    command=['C:/Users/EDY/.cargo/bin/cargo.exe',*args]
    p=subprocess.run(command,cwd=source,env=env,capture_output=True)
    (out/f'{i}.stdout').write_bytes(p.stdout);(out/f'{i}.stderr').write_bytes(p.stderr)
    rows.append({'command':command,'returncode':p.returncode})
    (out/'results.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
    print(p.stdout.decode('utf-8',errors='replace')[-1500:]+p.stderr.decode('utf-8',errors='replace')[-1800:],flush=True)
    if p.returncode:raise SystemExit(p.returncode)
for name in ['scheme.rs','xiaohe.rs']:shutil.copyfile(source/'crates/cloudleaf-pinyin/src'/name,here/name)

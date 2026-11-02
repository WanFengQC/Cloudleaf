"""在独立副本验证容量红绿与全部原搜索回归，保留每条命令原始字节。"""
from pathlib import Path
import hashlib,json,shutil,subprocess,time
base=Path(__file__).resolve().parent
source=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf/crates/cloudleaf-search')
probe=base/'validation';probe.mkdir(exist_ok=False)
shutil.copytree(source/'src',probe/'src')
shutil.copytree(source/'tests',probe/'tests')
(probe/'Cargo.toml').write_text('''[package]
name="cloudleaf-search"
version="0.1.0"
edition="2024"
rust-version="1.90.0"
publish=false
[dependencies]
cloudleaf-pinyin={path="C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf/crates/cloudleaf-pinyin"}
''',encoding='utf-8')
shutil.copyfile(base/'lexicon-red.rs',probe/'src/lexicon.rs')
cargo='C:/Users/EDY/.cargo/bin/cargo.exe'
commands=[]
def run(label,args,expected):
    started=time.perf_counter();result=subprocess.run(args,cwd=probe,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW)
    (base/(label+'.stdout')).write_bytes(result.stdout);(base/(label+'.stderr')).write_bytes(result.stderr)
    commands.append({'label':label,'command':args,'returncode':result.returncode,'expected':expected,'seconds':time.perf_counter()-started,
        'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
    (base/'commands.json').write_text(json.dumps(commands,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    assert result.returncode==expected,(label,result.stderr.decode(errors='replace'))
    print(label,result.returncode,flush=True)
run('red',[cargo,'+1.90.0','test','--lib','loaded_immutable_index_does_not_keep_growth_slack','--offline'],101)
shutil.copyfile(base/'lexicon-fixed.rs',probe/'src/lexicon.rs')
shutil.copyfile(base/'lib-fixed.rs',probe/'src/lib.rs')
run('fixed-search-regression',[cargo,'+1.90.0','test','--all-targets','--locked','--offline'],0)
run('fixed-clippy',[cargo,'+1.90.0','clippy','--all-targets','--locked','--offline','--','-D','warnings'],0)
example=probe/'examples';example.mkdir()
(example/'capacity.rs').write_text('''use cloudleaf_search::Lexicon;
fn main(){
 let text=std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
 let lex=Lexicon::from_tsv(&text).unwrap();
 println!("{:?}",lex.memory_estimate());
}
''',encoding='utf-8')
run('real-fixed-capacity',[cargo,'+1.90.0','run','--example','capacity','--release','--locked','--offline','--',
    'C:/Users/EDY/.codex/worktrees/cl008-search/Cloudleaf/.local/real-lexicon/cccedict-round4-2401b16/runtime.tsv'],0)
print((base/'real-fixed-capacity.stdout').read_text(encoding='utf-8'))

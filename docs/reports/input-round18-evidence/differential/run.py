from pathlib import Path
from datetime import datetime,timezone
import gzip,hashlib,json,subprocess,time,sys
root=Path(__file__).resolve().parent;repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=root/(sys.argv[1] if len(sys.argv)>1 else 'evidence');out.mkdir(exist_ok=False)
paths=[*sorted((repo/'crates/cloudleaf-search/src').glob('*.rs')),*sorted((repo/'crates/cloudleaf-pinyin/src').glob('*.rs'))]
hashes=lambda:{str(p.relative_to(repo)).replace('\\','/'):hashlib.sha256(p.read_bytes()).hexdigest()for p in paths}
source=hashes();head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip()
data='# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n'
entries=[('a','a','阿'),('a2','a','啊'),('an','an','安'),('na','na','那'),('ni','ni','你'),('ni2','ni','尼'),('nin','nin','您'),('hao','hao','好'),('hao2','hao','号'),('nong','nong','农'),('hu','hu','户'),('nonghu','nong hu','农户'),('nihao','ni hao','你好'),('nihao2','ni hao','尼号'),('zhong','zhong','中'),('guo','guo','国'),('ren','ren','人'),('guoren','guo ren','国人'),('zhongguo','zhong guo','中国'),('zhongguoren','zhong guo ren','中国人'),('bei','bei','北'),('jing','jing','京'),('da','da','大'),('xue','xue','学'),('beijing','bei jing','北京'),('daxue','da xue','大学'),('beida','bei jing da xue','北京大学'),('xi','xi','西'),('xian','xian','先'),('xian2','xian','鲜'),('xi-an','xi an','西安'),('lv','lv','绿'),('nv','nv','女'),('xin','xin','新'),('xing','xing','行'),('lang','lang','狼'),('lan','lan','蓝'),('nang','nang','囊'),('lo','lo','咯'),('luo','luo','罗')]
for name,pinyin,text in entries:data+=f'{name}\t{pinyin}\t{text}\t\tunknown\t\tmechanical-r18\t{name}\tMIT\n'
(root/'lexicon.tsv').write_text(data,encoding='utf-8',newline='\n');sha=hashlib.sha256(data.encode()).hexdigest()
record={'at_utc':datetime.now(timezone.utc).isoformat(),'source_head':head,'source_before':source,'lexicon_sha256':sha,'scope':'mechanical search correctness only; no language-quality or human-gold claim','commands':[]}
def command(label,args):
    start=time.perf_counter();p=subprocess.run(args,cwd=root,capture_output=True)
    (out/(label+'.stdout')).write_bytes(p.stdout);(out/(label+'.stderr')).write_bytes(p.stderr)
    row={'label':label,'command':args,'returncode':p.returncode,'seconds':time.perf_counter()-start,'stdout_sha256':hashlib.sha256(p.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(p.stderr).hexdigest()};record['commands'].append(row);print(json.dumps(row),flush=True)
    return p
try:
    cargo='C:/Users/EDY/.cargo/bin/cargo.exe'
    assert command('lock',[cargo,'generate-lockfile','--offline']).returncode==0
    assert command('build',[cargo,'build','--release','--locked','--offline']).returncode==0
    p=command('differential',[str(root/'target/release/r18-scheme-diff.exe'),str(root/'lexicon.tsv'),sha])
    raw=p.stdout;packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw
    (out/'differential.stdout.gz').write_bytes(packed)
    record['summary_lines']=[v for v in raw.decode('utf-8').splitlines()if v.startswith(('FAIL\t','SUMMARY\t'))]
    record['gzip']={'bytes':len(packed),'sha256':hashlib.sha256(packed).hexdigest(),'full_CRC_and_uncompressed_bytes_equal':True}
    record['run_passed']=p.returncode==0
finally:
    record['source_after']=hashes();record['source_unchanged']=record['source_before']==record['source_after'];record['head_after']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip()
    record['tools']={str(p.relative_to(root)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}for p in [root/'src/main.rs',root/'run.py',root/'Cargo.toml',root/'Cargo.lock',root/'lexicon.tsv',*sorted(root.glob('*.tsv'))]if p.exists()}
    (out/'summary.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
    print(json.dumps({k:record[k]for k in ['summary_lines','source_unchanged','run_passed']if k in record},ensure_ascii=False),flush=True)

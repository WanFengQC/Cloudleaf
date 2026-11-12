from pathlib import Path
import hashlib,json,re,subprocess
root=Path(__file__).parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
source=(repo/'crates/cloudleaf-pinyin/src/lib.rs').read_text(encoding='utf-8')
group=source.split('const SYLLABLE_GROUPS: &[&str] = &[',1)[1].split('];',1)[0]
syllables=sorted(set(' '.join(re.findall(r'"([a-z ]+)"',group)).split()))
code=(root/'xiaohe.rs').read_text(encoding='utf-8')
main='\nfn main(){ for syllable in ['+','.join(json.dumps(s)for s in syllables)+'] { println!("{}\\t{}",syllable,xiaohe_code(syllable).map(|v|String::from_utf8(v.to_vec()).unwrap()).unwrap_or_else(||"UNMAPPED".into()));}}\n'
(root/'all-syllables.rs').write_text(code+main,encoding='utf-8')
commands=[['C:/Users/EDY/.cargo/bin/rustc.exe','--edition','2024',str(root/'all-syllables.rs'),'-o',str(root/'all-syllables.exe')],[str(root/'all-syllables.exe')]]
runs=[]
for i,cmd in enumerate(commands):
    p=subprocess.run(cmd,capture_output=True);(root/f'verify-{i}.stdout').write_bytes(p.stdout);(root/f'verify-{i}.stderr').write_bytes(p.stderr);runs.append({'command':cmd,'returncode':p.returncode});assert p.returncode==0
rows=[line.split('\t')for line in p.stdout.decode().splitlines()]
reverse={}
for syllable,encoded in rows:
    assert encoded!='UNMAPPED',syllable
    assert len(encoded)==2 and encoded.isascii()and encoded.islower()
    reverse.setdefault(encoded,[]).append(syllable)
record={'scope':'Actual independently written encoder compiled against existing canonical legal syllable list; not language gold or third-party core','commands':runs,'repo_pinyin_sha256':hashlib.sha256((repo/'crates/cloudleaf-pinyin/src/lib.rs').read_bytes()).hexdigest(),'encoder_sha256':hashlib.sha256((root/'xiaohe.rs').read_bytes()).hexdigest(),'legal_syllables':len(rows),'all_mapped':True,'all_codes_two_ascii_letters':True,'collisions':{code:words for code,words in reverse.items()if len(words)>1},'mapping':dict(rows)}
(root/'syllable-verification.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in record.items()if k!='mapping'},ensure_ascii=False))

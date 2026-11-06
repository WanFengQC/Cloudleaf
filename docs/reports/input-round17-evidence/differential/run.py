from pathlib import Path
from datetime import datetime,timezone
import gzip
import hashlib
import json
import os
import statistics
import subprocess
import time

root=Path(__file__).resolve().parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
expected='3889c3d683053f4719fa5692996e0e31e214f3c1'
out=root/'evidence';out.mkdir(exist_ok=False)
(root/'src').mkdir(exist_ok=False)
(root/'Cargo.toml').write_text('[package]\nname="r17-actual-diff"\nversion="0.1.0"\nedition="2024"\nrust-version="1.90.0"\n[dependencies]\ncloudleaf-search={path="C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf/crates/cloudleaf-search"}\n[[bin]]\nname="r17-actual-diff"\npath="src/differential.rs"\n[[bin]]\nname="r17-actual-profile"\npath="src/profile.rs"\n[workspace]\n[profile.release]\nlto="thin"\ncodegen-units=1\n',encoding='utf-8',newline='\n')
(root/'rust-toolchain.toml').write_bytes((repo/'rust-toolchain.toml').read_bytes())
(root/'src/differential.rs').write_bytes(Path('C:/Users/EDY/.codex/tmp/r16-character-diff/src/main.rs').read_bytes())
(root/'src/profile.rs').write_bytes(Path('C:/Users/EDY/.codex/tmp/r16-phase-final-f2/src/main.rs').read_bytes())
(root/'cases.tsv').write_bytes(Path('C:/Users/EDY/.codex/tmp/r17-perf/cases.tsv').read_bytes())

paths=[*sorted((repo/'crates/cloudleaf-search/src').glob('*.rs')),repo/'crates/cloudleaf-search/Cargo.toml',repo/'crates/cloudleaf-pinyin/src/lib.rs',repo/'crates/cloudleaf-types/src/lib.rs']
def hashes():return {str(p.relative_to(repo)).replace('\\','/'):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
def head():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip()
before=hashes()
blob_match={name:hashlib.sha256(subprocess.check_output(['git','show',expected+':'+name],cwd=repo)).hexdigest()==value for name,value in before.items()}
assert all(blob_match.values()),blob_match
record={'expected_source_commit':expected,'head_before':head(),'at_utc':datetime.now(timezone.utc).isoformat(),'source_before':before,'source_matches_expected_blobs':blob_match,'commands':[]}
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(root/'target')
def command(label,args):
    start=time.perf_counter();result=subprocess.run(args,cwd=root,env=env,capture_output=True)
    (out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
    record['commands'].append({'label':label,'command':args,'returncode':result.returncode,'seconds':time.perf_counter()-start,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
    print(json.dumps({'label':label,'returncode':result.returncode,'seconds':record['commands'][-1]['seconds']}),flush=True)
    if result.returncode:raise RuntimeError(label+': '+result.stderr.decode('utf-8',errors='replace')[-2000:])
    return result.stdout
try:
    cargo='C:/Users/EDY/.cargo/bin/cargo.exe'
    command('lock',[cargo,'generate-lockfile','--offline'])
    command('build',[cargo,'build','--release','--locked','--offline'])
    raw=command('differential',[str(root/'target/release/r17-actual-diff.exe')])
    packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw
    (out/'differential.stdout.gz').write_bytes(packed)
    record['differential']={'raw_bytes':len(raw),'raw_sha256':hashlib.sha256(raw).hexdigest(),'gzip_bytes':len(packed),'gzip_sha256':hashlib.sha256(packed).hexdigest(),'gzip_CRC_and_exact_uncompressed_bytes':True,'summary_lines':[line for line in raw.decode().splitlines() if line.startswith('SUMMARY\t')]}
    assert hashlib.sha256(raw).hexdigest()=='e4134bfb68ea6e2fcc272189f9a6e79cee5d1cc6a2806808ebac06b5091f5939'
    runtime='C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv'
    sha='03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
    comparison=[];phase_checks=[]
    for kind,model in [('addone','character.tsv'),('witten','character-witten.tsv')]:
        model=str(repo/'.local/round16-language'/model)
        outputs={}
        for variant,exe in [('baseline',Path('C:/Users/EDY/.codex/tmp/r17-perf/baseline/target/release/r17-perf-probe.exe')),('actual',root/'target/release/r17-actual-profile.exe')]:
            args=[str(exe),runtime,str(root/'cases.tsv'),'exact','character',model,sha,'64','3','1','16384','200000','50000','10']
            stdout=command('profile-'+variant+'-'+kind,args)
            result=json.loads(stdout);outputs[variant]=result
            for case in result['cases']:
                for sample in case['samples']:
                    phases=sample['profile']['phases']
                    values={name:m['ns'] if m else 0 for name,m in phases.items()}
                    top=sum(values[name] for name in ['normalize','lattice','search_validate','traversal','final_sort_dedup','context_rerank','materialize'])
                    valid=sample['equal'] and sample['profile']['clock_reads']==2*sum(m['calls'] for m in phases.values() if m) and sample['profile_outer_ns']>=values['query_total']>=top and values['traversal']>=values['scoring']+values['bucket_sort_dedup']
                    phase_checks.append({'variant':variant,'kind':kind,'case':case['id'],'passed':valid})
        for a,b in zip(outputs['baseline']['cases'],outputs['actual']['cases']):
            same=a['id']==b['id'] and a['result']==b['result'] and a['result_debug']==b['result_debug']
            comparison.append({'kind':kind,'case':a['id'],'all_candidate_fields_and_statistics_equal':same,'baseline_median_ns':statistics.median(v['plain_ns'] for v in a['samples']),'actual_median_ns':statistics.median(v['plain_ns'] for v in b['samples']),'baseline_scoring_calls':a['samples'][0]['profile']['phases']['scoring']['calls'],'actual_scoring_calls':b['samples'][0]['profile']['phases']['scoring']['calls']})
        for variant in outputs:
            original=out/('profile-'+variant+'-'+kind+'.stdout')
            original.with_name(original.name+'.gz').write_bytes(gzip.compress(original.read_bytes(),mtime=0))
    assert len(comparison)==50 and all(r['all_candidate_fields_and_statistics_equal'] for r in comparison)
    assert len(phase_checks)==300 and all(r['passed'] for r in phase_checks)
    record['real_cases_comparisons']=comparison;record['phase_checks']=phase_checks
    record['profile_scope']='25 fixed cases including 12 R14 and13 mapped source-dev, baseline and actual same K10 and budgets; all phases pair/noop outputs equal. Exploratory window, background CPU not controlled, not InputEngine/Host/TSF performance.'
finally:
    record['source_after']=hashes();record['head_after']=head();record['source_unchanged']=before==record['source_after']
    record['source_after_matches_expected_blobs']={name:hashlib.sha256(subprocess.check_output(['git','show',expected+':'+name],cwd=repo)).hexdigest()==value for name,value in record['source_after'].items()}
    record['binaries']={p.name:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in (root/'target/release').glob('r17-actual-*.exe')}
    record['tool_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (out/'summary.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'source_unchanged':record['source_unchanged'],'head_before':record['head_before'],'head_after':record['head_after'],'differential':record['differential']['summary_lines'],'real_full_SearchResult_pairs':len(comparison),'phase_samples':len(phase_checks),'all_passed':all(r['passed'] for r in phase_checks)},indent=2))

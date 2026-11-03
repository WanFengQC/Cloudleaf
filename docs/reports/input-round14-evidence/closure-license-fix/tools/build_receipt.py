"""独立最终交付回执；不回写报告所处Git提交。"""
from pathlib import Path
from datetime import datetime,timezone
import hashlib
import json
import subprocess
root=Path(__file__).resolve().parents[1]
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip()
ci=json.loads((root/'.local/round14-ci/final-exact/summary.json').read_text(encoding='utf-8'))
downloads=json.loads((root/'.local/round14-ci/final-downloads/downloads.json').read_text(encoding='utf-8'))
prs=json.loads((root/'.local/round14-prs-final.json').read_text(encoding='utf-8'))
summary=json.loads((root/'docs/reports/input-round14-evidence/summary.json').read_text(encoding='utf-8'))
closure=json.loads((root/'docs/reports/input-round14-evidence/closure-license-fix/summary.json').read_text(encoding='utf-8'))
assert ci['verified_pass'] and ci['head_sha']==head==downloads['head']
assert subprocess.check_output(['git','status','--porcelain'],cwd=root)==b''
assert subprocess.check_output(['git','rev-parse','origin/codex/input-round14'],cwd=root).decode().strip()==head
assert subprocess.check_output(['git','rev-parse','origin/main'],cwd=root).decode().strip()=='a564fd47b0f054ff360592af3ccb8d744021160f'
expected={1:'83a2666fa748b4ae9cba41a5880ba5353e71c9a3',2:'59dabdc2f8a3dc87cbd7f19ed89401ee607f7954',
          3:'3f26f34e00c0775124f87e150a021f99fb52c567',4:'e0ab78c2786d652a3a19cb544b4dc83996e63264',
          5:'cca77457ca3943ee5ad90d566a8d9ef6b8795cd2'}
assert all(row['state']=='open' and row['draft'] and row['head']==expected[row['number']] and not row['merged'] for row in prs['pulls'])
assert not subprocess.check_output(['git','diff',closure['implementation_head'],head,'--','crates','apps','scripts','config','tools','tests','Cargo.lock','Cargo.toml','.gitattributes','data/licenses','data/sources'],cwd=root)
assert not subprocess.check_output(['git','diff',summary['implementation_head'],head,'--','crates','apps','Cargo.lock','Cargo.toml'],cwd=root)
files=['docs/reports/input-round14-status.md','docs/reports/input-round14-evidence/index.json','Cargo.lock',
       'docs/input-round14-design.md','.local/round14-closure-lexicon-receipt.json','.local/round14-prs-final.json',
       'docs/reports/input-round14-evidence/closure-license-fix/summary.json',
       '.local/round14-ci/final-exact/summary.json','.local/round14-ci/final-downloads/downloads.json',
       '.local/round14-final-history-preservation.json']
receipt={'schema':1,'round':14,'status':'partial','at_utc':datetime.now(timezone.utc).isoformat(),
    'branch':'codex/input-round14','final_head':head,'base':'83a2666fa748b4ae9cba41a5880ba5353e71c9a3',
    'implementation_head':closure['implementation_head'],'input_implementation_head':summary['implementation_head'],
    'source_identical_to_full_local_checks':True,'Rust_product_sources_unchanged_since_input_measurements':True,
    'new_PR_created':False,'existing_PRs_unchanged':prs,'main_unchanged':True,'working_tree_clean':True,
    'history_preservation':json.loads((root/'.local/round14-final-history-preservation.json').read_text(encoding='utf-8')),
    'push_CI':ci,'downloaded_CI':downloads,'local_input_implementation':summary,'local_final_checks':closure,
    'real_data':json.loads((root/'.local/round14-closure-lexicon-receipt.json').read_text(encoding='utf-8')),
    'important_observed_failures':{'target_sentence_first_candidate':'握正在炌法医宽中文输入法',
        'target_expected_in_top5_or10':False,'formal_language_metrics':{'top1':None,'top5':None,'mrr':None},
        'input_handle_engineering_p95_ms':6.0604,'target_p95_ms':5,'latency_target_passed':False},
    'fixed_issues':['native candidate blank-area click selected an unseen next-page candidate',
        'immutable lexicon retained 23538688 bytes avoidable vector growth capacity',
        'v2 nullable frequency and search integration gaps',
        'source-license working-tree newline drift now restored and checked against frozen locks'],
    'not_run':['TSF registration/activation/install','actual Windows TSF app acceptance','8h/24h soak','physical network disconnect','DPI and multi-monitor acceptance'],
    'not_implemented':['local unknown fragment mixed fallback','search cancellation','incremental search','correction/fuzzy/abbreviated/double pinyin','learning/user dictionary','formal context model integration'],
    'blocked':['modern frequency and independent gold','natural Chinese ranking quality','existing Windows reliability/environment approval gates'],
    'CL010':'closed','translation_branch_modified':False,
    'file_SHA256':{name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in files},
    'tool_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
destination=root/'.local/round14-delivery-receipt.json'
with destination.open('x',encoding='utf-8',newline='\n') as file:json.dump(receipt,file,ensure_ascii=False,indent=2);file.write('\n')
print(json.dumps({'head':head,'status':'partial','exact_push_CI':ci['url'],'all_five_Draft':True,'receipt':str(destination)},ensure_ascii=False))

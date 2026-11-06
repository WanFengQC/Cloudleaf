"""只复核已冻结原件和任务，不调用候选搜索，也不改变样本。"""
from pathlib import Path
from collections import Counter
from html.parser import HTMLParser
import hashlib
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parent
OUT = ROOT / 'final-audit-v2'
OUT.mkdir(exist_ok=False)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def load(path):
    return json.loads(path.read_text(encoding='utf-8'))

def rows(path):
    return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]

def write(name, value):
    (OUT/name).write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n', encoding='utf-8', newline='\n')

source = ROOT / 'wikinews-originals'
manifest = load(source/'acquisition-manifest.json')
assert sha((source/'acquisition-manifest.json').read_bytes()) == 'b9fc2992b6a097345483055ad56728fe75054bb65307d6ae0aca1eef4cbcb165'
source_files = {}
for name, expected in manifest['files'].items():
    data = (source/name).read_bytes()
    assert len(data) == expected['bytes'] and sha(data) == expected['sha256'], name
    source_files[name] = {'bytes': len(data), 'sha256': sha(data)}
for request in manifest['requests']:
    assert sha((source/request['file']).read_bytes()) == request['sha256']

stable_files = ['article-screening.json', 'dev-events.tsv', 'heldout-events.tsv', 'sample-freeze-before-mapping.jsonl', 'tasks.jsonl']
replay = {}
for name in stable_files:
    a = (ROOT/'wikinews-tasks-a'/name).read_bytes()
    b = (ROOT/'wikinews-tasks-b'/name).read_bytes()
    assert a == b, name
    replay[name] = {'byte_identical': True, 'bytes': len(a), 'sha256': sha(a)}
sa = load(ROOT/'wikinews-tasks-a'/'summary.json')
sb = load(ROOT/'wikinews-tasks-b'/'summary.json')
sa.pop('created_utc'); sb.pop('created_utc')
assert sa == sb
write('double-build.json', {'stable_files': replay, 'summaries_equal_except_created_utc': True})

tasks = rows(ROOT/'wikinews-tasks-a'/'tasks.jsonl')
dev = [r for r in tasks if r['split'] == 'dev']
ready = [r for r in dev if r['mapping_status'] == 'ready']
assert len(ready) == 40 and len(dev) == 48
for row in tasks:
    assert row['human_confirmed_pinyin'] is None
    assert row['human_intent'] is None and row['acceptable_outputs'] is None
    assert row['formal_gold'] is False
    assert not row.get('tatoeba_train_exact_overlap', False)
    assert not row.get('tatoeba_train_near_overlap', [])
split_families = {split: {r['article_family'] for r in tasks if r['split']==split} for split in ['dev','heldout']}
assert not split_families['dev'] & split_families['heldout']
target_text = 'case\tpinyin\tsource_text\n' + ''.join(r['case']+'\t'+r['derived_pinyin']+'\t'+r['source_text']+'\n' for r in ready)
(OUT/'dev-targets.tsv').write_text(target_text, encoding='utf-8', newline='\n')
(OUT/'dev-tasks.jsonl').write_text(''.join(json.dumps(r,ensure_ascii=False,sort_keys=True)+'\n' for r in dev), encoding='utf-8', newline='\n')
write('dev-denominators.json', {
    'slots':48, 'statuses':dict(Counter(r['mapping_status'] for r in dev)),
    'ready_baseline_complete_paths':sum(r['baseline_complete_path'] for r in ready),
    'ready_supplemented_complete_paths':sum(r['combined_complete_path'] for r in ready),
    'all_frozen_source_baseline_complete_paths':sum(r.get('baseline_complete_path',False) for r in dev),
    'all_frozen_source_supplemented_complete_paths':sum(r.get('combined_complete_path',False) for r in dev),
    'failed_slots':[{'case':r['case'],'mapping_status':r['mapping_status'],'source_text':r['source_text'],'source_pageid':r['source_pageid'],'copyright_exception_flags':r['copyright_exception_flags'],'derived_input_bytes':r.get('derived_input_bytes')} for r in dev if r['mapping_status']!='ready'],
    'reading_ambiguity_cases':sum(bool(r.get('reading_ambiguities')) for r in ready),
    'new_corpus_training_used':False, 'heldout_candidates_executed':False,
    'formal_metrics':{'top1':None,'top5':None,'mrr':None},
    'dev_targets_sha256':sha(target_text.encode()), 'human_gold':False,
})

article_screening = load(ROOT/'wikinews-tasks-a'/'article-screening.json')
source_review = []
for article in manifest['articles']:
    pageid = article['pageid']
    revision_doc = load(source/f'page-{pageid}-revision.json')
    revision = revision_doc['query']['pages'][0]['revisions'][0]
    assert revision['revid'] == article['revision_id']
    assert revision['sha1'] == article['revision_sha1']
    parsed = load(source/f'page-{pageid}-rendered-original.json')['parse']
    assert parsed['revid'] == revision['revid']
    screening = next(r for r in article_screening if r['pageid']==pageid)
    source_review.append({
        'pageid':pageid,'title':article['title'],'revision_id':revision['revid'],
        'revision_timestamp':revision['timestamp'],'category_timestamp':next(r['timestamp'] for r in load(source/'selection-before-content.json')['preselected'] if r['pageid']==pageid),
        'article_date_templates':screening['article_date_templates'],
        'revision_url':article['revision_url'],'history_url':article['history_url'],
        'attribution':article['attribution'],'license_id':'CC-BY-4.0',
        'source_exception_flags':screening['copyright_exception_flags'],
        'contributors_list_complete':article['contributors_complete'],
        'usage':'research_only','production_approved':False,
        'modifications':['保留原始字形，从正文提取不含引号或联络形状的6至24汉字片段；不保留全部上下文','由显式研发词典映射拼音，可能选非常用读音；非人工gold'],
    })
write('source-review.json',{
    'schema_version':1,'source_family':'wikimedia-wikinews-zh',
    'fixed_acquisition_manifest_sha256':sha((source/'acquisition-manifest.json').read_bytes()),
    'source_files_verified':len(source_files),'raw_file_bytes_verified':sum(r['bytes'] for r in source_files.values()),
    'current_policy_evidence':{'copyright_url':'https://zh.wikinews.org/zh-hans/Wikinews:%E7%89%88%E6%9D%83%E4%BF%A1%E6%81%AF','copyright_sha256':sha((source/'copyright.html').read_bytes()),'policy_text_locations':'copyright.html lines634-636 and footer837; current CC-BY-4.0 text, before2025-01-30 also previously CC-BY-2.5','reuse_sha256':sha((source/'reuse.html').read_bytes()),'reuse_discrepancy':'旧转载说明仍有CC-BY-2.5例子；本轮采用当前版权方针与页脚4.0，保留旧文字而不伪造一致','cc_by_4_legalcode_sha256':sha((source/'cc-by-4.0-legalcode.html').read_bytes()),'terms_sha256':sha((source/'wikimedia-terms.html').read_bytes())},
    'permitted_research_scope':'仅固定页面中通过保守筛选的文字片段；保留来源、修订、历史与署名，提供许可链接和修改说明。图像、音视频、引文、联络形状段落及例外flag均不导入。',
    'production_approved':False,'legal_human_approval':None,
    'limitations':['站点许可不证明每个第三方引文、个人权利均获逐一审查；本轮只作有界研发使用','页面53152原文含侵權这一主题词，保守关键词规则因此隔离3个槽；这是规则false positive而非已确认侵权，冻结后不补位','环境主题分类返回0条；7分类不等于最终7领域均衡覆盖','分类时间不是文章发布时间；已选文章正文日期跨2020-10-01至2026-01-05','初始脚本未保存部分HTTP响应头；固定原始字节及恢复后的完整清单仍在，不补造初始头'],
    'articles':source_review,
})

diagnosis_b=rows(ROOT/'traditional-c'/'r16-all32-diagnosis.jsonl')
diagnosis_c=rows(ROOT/'traditional-d'/'r16-all32-diagnosis.jsonl')
assert diagnosis_b==diagnosis_c
assert len(diagnosis_b)==32
missing=[r for r in diagnosis_b if not r['baseline_path_exists']]
assert len(missing)==19 and all(r['combined_path_exists'] for r in missing)
assert all(r['pinyin_parser_failed'] is None for r in diagnosis_b)
write('r16-all32-review.json',{
    'diagnosis_c_sha256':sha((ROOT/'traditional-c'/'r16-all32-diagnosis.jsonl').read_bytes()),
    'diagnosis_d_byte_identical':True,'all32':32,'baseline_complete':13,'supplemented_complete':32,
    'nineteen_unmappable':19,'nineteen_with_missing_character_evidence':sum(bool(r['baseline_missing_characters']) for r in missing),
    'input_bytes_max':max(r['combined_input_bytes'] for r in diagnosis_b),
    'parser_failures_demonstrated':None,'rust_lattice_verification':'root separately ran actual Rust coverage_probe; this script does not claim parser/search execution',
    'unknown_frequency_preserved':True,'human_reading_gold':False,
    'reading_ambiguity_cases':sum(r['reading_ambiguity_present'] for r in diagnosis_b),
    'examples_of_non_gold_readings':[{'case':case,'pinyin':next(r['combined_pinyin'] for r in diagnosis_b if r['case']==case),'note':note} for case,note in [('r16dev001','的可被stable ID选为di'),('r16dev011','數可被选为shuo'),('r16dev013','傳授可被选为zhuanshou'),('r16dev032','說話可被选为shuihua')]],
    'cases':[{'case':r['case'],'baseline_complete':r['baseline_path_exists'],'supplemented_complete':r['combined_path_exists'],'missing_characters':r['baseline_missing_characters'],'input_bytes':r['combined_input_bytes'],'ambiguous_readings':r['reading_ambiguity_present']} for r in diagnosis_b],
    'coverage_not_quality':True,
})
write('audit-identity.json',{'files':{p.name:{'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in OUT.iterdir() if p.is_file()},'tool_sha256':sha(Path(__file__).read_bytes()),'source_scripts':{name:sha((ROOT/name).read_bytes()) for name in ['prepare_wikinews_tasks.py','prepare_traditional_layer.py','resume_wikinews.py']},'root_repo_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=r'C:\Users\EDY\.codex\worktrees\input-round14\Cloudleaf',text=True).strip(),'candidate_execution':False})
print(json.dumps({'audit_files':len(list(OUT.iterdir())),'dev_targets_sha256':sha(target_text.encode()),'ready_dev':len(ready),'source_files_verified':len(source_files),'double_build_equal':True},indent=2))

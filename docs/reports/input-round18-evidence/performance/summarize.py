from pathlib import Path
import json,hashlib
out=Path(__file__).resolve().parent;s=json.loads((out/'summary.json').read_text(encoding='utf-8'))
lines=['# Round18 固定真实词典 K10 研发观测','',f"源码HEAD：`{s['implementation_head']}`。源码前后不变：{s['source_unchanged']}。全为engineering-only，正式Top1/5/MRR为null。",'',s['scope'],'','## 默认全拼相同171事件配对','', '|模式|n|P50 ms|P95 ms|P99 ms|max ms|','|---|---:|---:|---:|---:|---:|']
for mode,row in s['default_performance_ns'].items():lines.append(f"|{mode}|{row['n']}|"+'|'.join(f"{row[k]/1e6:.4f}"for k in ['p50','p95','p99','max'])+'|')
lines+=['','15组完整输出一致，只移除新增加且值严格为零/None的兼容元数据；未归一化正文、ID、路径、分数、统计或非零解释成本。没有丢弃首轮或长尾。','', '## 新方案逐键引擎观测','', '|方案/模型|逐键n|P50 ms|P95 ms|P99 ms|max ms|','|---|---:|---:|---:|---:|---:|']
for mode,row in s['scheme_event_performance_ns'].items():lines.append(f"|{mode}|{row['n']}|"+'|'.join(f"{row[k]/1e6:.4f}"for k in ['p50','p95','p99','max'])+'|')
lines+=['','新方案三轮/模型；full/abbreviated/fuzzy/correction每轮20个最终案例，xiaohe13。全部最终profile查询837，预算失败90（30个唯一方案/模型/案例×3），均为Abbreviated+字符两模型Expansions200000。InputEngine预算失败时不提供局部选择，原文保留。所有中间逐键延迟保存，但scheme_trace的EVENT只打印error，不逐键打印budget；这里预算分母专指最终输入profile，不能冒称中间每键都无预算。','', '## 实际Top10观察','', '均为来源候选观察，不是gold；完整ID/Features/来源/评分保留原始RESULT与DIAGNOSTIC。`raw-unknown`仅历史测试标签，例如小鹤zz实际可解释为zou，不能据标签认定非法。','', '|方案|模型|输入案例|Top10正文|预算|','|---|---|---|---|---|']
for row in s['candidate_observations']:
    if row['case'] in ['zgr','bjdx','zg','zhg','zhonggr','wzgs','nihao','nh','xiaohe-nihao','xiaohe-zhongguoren','xiaohe-beijingdaxue','fuzzy-lin','fuzzy-lilan','typo-adjacent','typo-transpose','typo-long','collision-lo']:
        lines.append('|'+row['scheme']+'|'+row['model']+'|'+row['case']+'|'+' / '.join(row['texts_top10'])+'|'+('Expansions'if'budget: Some' in row['statistics']else'None')+'|')
(out/'observations.md').write_text('\n'.join(lines)+'\n',encoding='utf-8',newline='\n')
lines=['# 显式阶段观测','', '普通引擎不读这些诊断时钟；profile程序另外运行plain query后再运行带观测query并严格断言完整结果相同。阶段计时包含观察器开销，不能与逐键或Host测量直接混比。scoring/bucket嵌套在traversal内，全部阶段不能相加。full默认关闭方案解释，其解释profile全零表示未运行。','', '|方案/模型/阶段|n|P50 ms|P95 ms|P99 ms|max ms|','|---|---:|---:|---:|---:|---:|']
for mode,row in {**s['scheme_phase_ns'],**s['scheme_interpretation_phase_ns']}.items():lines.append(f"|{mode}|{row['n']}|"+'|'.join(f"{row[k]/1e6:.4f}"for k in ['p50','p95','p99','max'])+'|')
(out/'phases.md').write_text('\n'.join(lines)+'\n',encoding='utf-8',newline='\n')
index={str(p.relative_to(out)).replace('\\','/'):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}for p in out.rglob('*')if p.is_file()and p.name!='index.json'and p.suffix!='.exe'}
(out/'index.json').write_text(json.dumps({'source_head':s['implementation_head'],'files':index},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'files':len(index),'bytes':sum(r['bytes']for r in index.values()),'source_unchanged':s['source_unchanged'],'default_pairs_all_equal':all(p['full_retained_output_equal_after_only_zero_new_fields']for p in s['default_pairs']),'runs':len(s['runs'])}))

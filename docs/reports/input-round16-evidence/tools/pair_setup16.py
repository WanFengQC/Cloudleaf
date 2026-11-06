from pathlib import Path
root=Path(__file__).resolve().parents[1]
s=(root/'.local/paired_traces15.py').read_text(encoding='utf-8')
s=s.replace('round15-paired-traces','round16-paired-traces').replace('trace15.py','trace16.py').replace("f'round15-final-", "f'round16-final-")
s=s.replace("variants = ['original', 'structural', 'lexical']", "variants = ['original', 'originallexical', 'structural', 'lexical', 'character']")
s=s.replace('iteration % 3', 'iteration % len(variants)')
s=s.replace("    assert stable_output(records['original']) == stable_output(records['structural']), '原结构后端输出或工作统计发生变化'", "    assert stable_output(records['original']) == stable_output(records['structural']), '原结构后端输出或工作统计发生变化'\n    assert stable_output(records['originallexical']) == stable_output(records['lexical']), '旧词汇后端发生变化'")
s=s.replace('75e0acee88d21647010a56a9d876c136c95e6d5e', '5045e18981dadefd3881a05fd021b31712ac243e')
s=s.replace("'model_sha256':", "'old_lexical_output_and_statistics_identical': True,\n    'character_model_sha256': hashlib.sha256((root/'.local/round16-language/character.tsv').read_bytes()).hexdigest(),\n    'model_sha256':")
(root/'.local/paired_traces16.py').write_text(s,encoding='utf-8',newline='\n')
for stem in ['collect_ci','download_ci']:
    s=(root/f'.local/{stem}15.py').read_text(encoding='utf-8').replace('round15','round16').replace('Round15','Round16').replace('R15','R16')
    (root/f'.local/{stem}16.py').write_text(s,encoding='utf-8',newline='\n')

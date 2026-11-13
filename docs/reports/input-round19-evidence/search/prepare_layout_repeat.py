from pathlib import Path
p=Path(__file__).resolve().parent
s=(p/'paired_finish.py').read_text(encoding='utf-8')
s=s.replace("out=p/'paired-final'","out=p/'paired-layout-exploration'")
s=s.replace("'addone':root/'.local/round16-language/character.tsv','witten':root/'.local/round16-language/character-witten.tsv'",'')
s=s.replace("'new':p/'source/target/release/r19-search-probe.exe'","'new':p/'source/target/release/r19-search-probe.exe','layout':p/'layout-exploration/target/release/r19-search-probe.exe'")
s=s.replace('range(1,4)','range(1,8)')
s=s.replace("(['old','new']if iteration%2 else['new','old'])","(['old','layout','new']if iteration%2 else['new','layout','old'])")
s=s.replace('3freshprocesses/condition','7freshprocesses/condition; middle variant oldsearch plus new sharedlayout')
(p/'layout_repeat.py').write_text(s,encoding='utf-8')

from pathlib import Path
root=Path(__file__).resolve().parents[1]
header=(root/'.local/round16-language/metadata.tsv').read_text(encoding='utf-8').replace('kind\tcharacter_bigram\n','kind\tcharacter_bigram_witten_bell\n')
(root/'.local/round16-language/witten-metadata.tsv').write_text(header,encoding='utf-8',newline='\n')
p=root/'.local/trace16.py';s=p.read_text(encoding='utf-8').replace("['character','covered','word']", "['character','covered','word','witten']")
s=s.replace("else '.local/round16-language/word.tsv')", "else '.local/round16-language/character-witten.tsv' if variant=='witten' else '.local/round16-language/word.tsv')")
p.write_text(s,encoding='utf-8',newline='\n')

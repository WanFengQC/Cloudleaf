from pathlib import Path
import shutil

root=Path(__file__).resolve().parents[1]
old=Path('C:/Users/EDY/.codex/tmp/r16-phase-profile')
new=Path('C:/Users/EDY/.codex/tmp/r16-phase-final-f2')
new.mkdir(exist_ok=False)
(new/'src').mkdir()
for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','cases.tsv','compile.py','src/main.rs']:
    shutil.copyfile(old/name,new/name)
s=(old/'measure.py').read_text(encoding='utf-8')
s=s.replace("character = root / '.local/round16-language/character.tsv'", "character = root / '.local/round16-language/character.tsv'\nwitten = root / '.local/round16-language/character-witten.tsv'")
s=s.replace("('structural', 'lexical', 'character')", "('structural', 'lexical', 'character', 'witten')")
s=s.replace("model = character if ranker == 'character' else lexical", "model = witten if ranker == 'witten' else character if ranker == 'character' else lexical")
s=s.replace("str(cases), method, ranker, str(model)", "str(cases), method, 'character' if ranker == 'witten' else ranker, str(model)")
s=s.replace("== 144", "== 192")
s=s.replace("'models': {'lexical':", "'models': {'witten': {'path': str(witten), 'sha256': sha(witten)}, 'lexical':")
(new/'measure.py').write_text(s,encoding='utf-8',newline='\n')
s=(root/'.local/paired_traces16.py').read_text(encoding='utf-8').replace('round16-paired-traces','round16-delivery-paired-traces').replace("f'round16-final-", "f'round16-delivery-")
s=s.replace("'lexical', 'character']", "'lexical', 'character', 'witten']")
(root/'.local/paired_delivery16.py').write_text(s,encoding='utf-8',newline='\n')
s=(root/'.local/summarize16.py').read_text(encoding='utf-8').replace('round16-summary','round16-delivery-summary').replace('round16-paired-traces','round16-delivery-paired-traces').replace('round16-verified-results','round16-delivery-results').replace('verified-04','delivery-04').replace('verified-08','delivery-08').replace('8a0838b2261d0cf74f1ae17fa028849509deedc6','f2b5c5ee53882787c84b29180da6e835ffa98a27')
s=s.replace("['lexical','character','covered','word']", "['lexical','character','covered','word','witten']")
s=s.replace("'covered-character.tsv','word.tsv']", "'covered-character.tsv','word.tsv','character-witten.tsv']")
(root/'.local/summarize_delivery16.py').write_text(s,encoding='utf-8',newline='\n')
print('new final profile, paired traces and summaries prepared')

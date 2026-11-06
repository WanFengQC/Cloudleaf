from pathlib import Path
root=Path(__file__).resolve().parents[1]
s=(root/'.local/trace16.py').read_text(encoding='utf-8')
s=s.replace('round16-baseline-build','round17-baseline-build').replace('5045e18981dadefd3881a05fd021b31712ac243e','521472db18ecd7926f6e5b58b0afd7b5da674a07')
s=s.replace("if variant in ['lexical','originallexical']", "if variant in ['lexical','originallexical']")
s=s.replace("if variant in ['character','covered','word','witten']:", "if variant in ['character','originalcharacter','witten','originalwitten']:")
start=s.index("    model=root/('.local/round16-language/character.tsv'")
end=s.index('\nevents=',start)
s=s[:start]+"    model=root/('.local/round16-language/character-witten.tsv' if 'witten' in variant else '.local/round16-language/character.tsv')\n    command.extend([str(model),hashlib.sha256(model.read_bytes()).hexdigest(),'character_bigram'])\n"+s[end:]
(root/'.local/trace17.py').write_text(s,encoding='utf-8',newline='\n')
s=(root/'.local/paired_delivery16.py').read_text(encoding='utf-8').replace('round16-delivery','round17-final').replace('trace16.py','trace17.py').replace('5045e18981dadefd3881a05fd021b31712ac243e','521472db18ecd7926f6e5b58b0afd7b5da674a07')
s=s.replace("['original', 'originallexical', 'structural', 'lexical', 'character', 'witten']", "['originalcharacter', 'character', 'originalwitten', 'witten', 'originallexical', 'lexical']")
a=s.index("    assert stable_output(records['original'])")
b=s.index('\ndef case_details',a)
s=s[:a]+"    for kind in ['character','witten','lexical']:\n        assert stable_output(records['original'+kind]) == stable_output(records[kind]), kind+' complete output and statistics changed'\n"+s[b:]
s=s.replace("('FRAME\\t', 'CANDIDATE\\t', 'COMMIT\\t', 'SEARCH\\t')", "('FRAME\\t', 'CANDIDATE\\t', 'COMMIT\\t', 'SEARCH\\t','PATH\\t')")
(root/'.local/paired17.py').write_text(s,encoding='utf-8',newline='\n')
s=(root/'.local/full16_delivery.py').read_text(encoding='utf-8').replace('round16','round17').replace('run16.py','run17.py')
(root/'.local/full17.py').write_text(s,encoding='utf-8',newline='\n')
for name in ['collect_ci','download_ci','discover']:
    s=(root/f'.local/{name}16.py').read_text(encoding='utf-8').replace('round16','round17').replace('Round16','Round17').replace('R16','R17')
    (root/f'.local/{name}17.py').write_text(s,encoding='utf-8',newline='\n')

from pathlib import Path
root=Path(__file__).resolve().parents[1]
p=root/'.local/full19_final.py'
assert not p.exists()
s=(root/'.local/full19.py').read_text(encoding='utf-8').replace("f'verified-{i:02}'","f'final-{i:02}'").replace('round19-verified','round19-final')
p.write_text(s,encoding='utf-8')

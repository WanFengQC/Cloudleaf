from pathlib import Path
root=Path(__file__).resolve().parents[1]
p=root/'.local/full19_locked.py';assert not p.exists()
s=(root/'.local/full19_final.py').read_text(encoding='utf-8').replace("f'final-{i:02}'","f'locked-{i:02}'").replace('round19-final','round19-locked')
p.write_text(s,encoding='utf-8')

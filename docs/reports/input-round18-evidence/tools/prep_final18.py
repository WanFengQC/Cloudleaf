from pathlib import Path
root=Path(__file__).resolve().parents[1]
s=(root/'.local/full18.py').read_text(encoding='utf-8').replace("f'delivery-{i:02}'","f'verified-{i:02}'").replace('round18-delivery-results','round18-verified-results').replace('round18-final-dependency-audit','round18-verified-dependency-audit').replace('round18-performance','round18-verified-performance').replace('round18-algorithm-v1','round18-verified-algorithm-v1')
(root/'.local/full18_verified.py').write_text(s,encoding='utf-8',newline='\n')
for name in ['collect_ci','download_ci','discover']:
    s=(root/f'.local/{name}17.py').read_text(encoding='utf-8').replace('round17','round18').replace('Round17','Round18').replace('R17','R18')
    (root/f'.local/{name}18.py').write_text(s,encoding='utf-8',newline='\n')

from pathlib import Path
root=Path(__file__).resolve().parents[1]
s=(root/'.local/full16.py').read_text(encoding='utf-8').replace("label=f'final-{i:02}'", "label=f'delivery-{i:02}'").replace('round16-full-results.json','round16-delivery-results.json')
(root/'.local/full16_delivery.py').write_text(s,encoding='utf-8',newline='\n')

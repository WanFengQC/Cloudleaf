from pathlib import Path
import hashlib
import io
import json
import subprocess
import zipfile

root=Path(__file__).resolve().parents[1]
s=(root/'.local/trace15.py').read_text(encoding='utf-8').replace('round15-baseline-build','round16-baseline-build')
s=s.replace("events=(root/'tests/fixtures/input-round14-events.tsv').read_bytes()", """if variant=='character':
    model=root/'.local/round16-language/character.tsv'
    command.extend([str(model),hashlib.sha256(model.read_bytes()).hexdigest(),'character_bigram'])
events=(root/(sys.argv[3] if len(sys.argv)>3 else 'tests/fixtures/input-round14-events.tsv')).read_bytes()""")
s=s.replace("'75e0acee88d21647010a56a9d876c136c95e6d5e'", "'5045e18981dadefd3881a05fd021b31712ac243e'")
(root/'.local/trace16.py').write_text(s,encoding='utf-8',newline='\n')
source=root/'.local/round16-baseline-source'
source.mkdir(exist_ok=False)
data=subprocess.check_output(['git','archive','--format=zip','5045e18981dadefd3881a05fd021b31712ac243e'],cwd=root)
with zipfile.ZipFile(io.BytesIO(data)) as archive:
    archive.extractall(source)
record={'commit':'5045e18981dadefd3881a05fd021b31712ac243e','archive_sha256':hashlib.sha256(data).hexdigest(),'source':str(source)}
(root/'.local/round16-baseline-source.json').write_text(json.dumps(record,indent=2),encoding='utf-8')

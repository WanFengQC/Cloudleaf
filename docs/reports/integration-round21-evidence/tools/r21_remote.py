"""读取原五个 PR 的公开状态，避免把凭据写入证据。"""
import json
import subprocess
import sys
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parents[1]
credential = subprocess.run(['git', '-c', 'credential.interactive=never', 'credential', 'fill'],
                            input=b'protocol=https\nhost=github.com\n\n', capture_output=True, check=True, timeout=30)
token = dict(line.split('=', 1) for line in credential.stdout.decode().splitlines() if '=' in line)['password']
api = 'https://api.github.com/repos/WanFengQC/Cloudleaf'
headers = {'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
           'User-Agent': 'Cloudleaf-Round21-verification', 'Cache-Control': 'no-cache'}
prs = []
for num in range(1, 6):
    request = urllib.request.Request(f'{api}/pulls/{num}', headers=headers)
    with urllib.request.urlopen(request, timeout=45) as response:
        data = json.load(response)
    prs.append({'number': num, 'state': data['state'], 'draft': data['draft'], 'merged': data['merged'],
                'head': data['head']['sha'], 'base': data['base']['ref'], 'base_sha': data['base']['sha']})
label = sys.argv[1]
assert label in {'before-push', 'final'}
with (root / '.local' / f'r21-{label}-prs.json').open('x', encoding='utf-8', newline='\n') as stream:
    json.dump(prs, stream, ensure_ascii=False, indent=2)
    stream.write('\n')
assert prs == json.loads((root / '.local/r21-original-prs.json').read_text(encoding='utf-8'))
print(json.dumps(prs))

"""只读核对当前PR及固定CI，不写仓库。"""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.request

root = Path(__file__).parent
result = subprocess.run(['git', '-c', 'credential.interactive=never', 'credential', 'fill'], input=b'protocol=https\nhost=github.com\n\n', capture_output=True, timeout=30)
if result.returncode:
    raise RuntimeError('现有Git凭据不可用')
credential = dict(line.split('=', 1) for line in result.stdout.decode().splitlines() if '=' in line)
headers = {'Accept': 'application/vnd.github+json', 'User-Agent': 'Cloudleaf-Round16-readonly-review', 'Authorization': 'Bearer ' + credential['password']}
api = 'https://api.github.com/repos/WanFengQC/Cloudleaf'

def fetch(pair):
    suffix, name = pair
    with urllib.request.urlopen(urllib.request.Request(api + suffix, headers=headers), timeout=45) as response:
        raw = response.read()
    (root / name).write_bytes(raw)
    return name, json.loads(raw), hashlib.sha256(raw).hexdigest()

queries = [(f'/pulls/{number}', f'pr{number}.json') for number in range(1, 6)] + [('/actions/runs/42156523761', 'round15-run.json'), ('/actions/runs/42156523761/jobs?per_page=100', 'round15-jobs.json'), ('/branches/main', 'main.json')]
with ThreadPoolExecutor(max_workers=5) as pool:
    rows = list(pool.map(fetch, queries))
values = {name: value for name, value, _ in rows}
summary = {'at_utc': datetime.now(timezone.utc).isoformat(), 'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'raw_sha256': {name: sha for name, _, sha in rows}, 'pulls': [{'number': values[f'pr{i}.json']['number'], 'head': values[f'pr{i}.json']['head']['sha'], 'state': values[f'pr{i}.json']['state'], 'draft': values[f'pr{i}.json']['draft'], 'merged': values[f'pr{i}.json']['merged']} for i in range(1, 6)], 'main': values['main.json']['commit']['sha'], 'round15_run': {key: values['round15-run.json'][key] for key in ('id', 'head_sha', 'event', 'status', 'conclusion', 'html_url')}, 'round15_jobs': [{'name': v['name'], 'head_sha': v['head_sha'], 'status': v['status'], 'conclusion': v['conclusion'], 'all_steps_success_or_skipped': all(s['conclusion'] in ('success', 'skipped') for s in v['steps'])} for v in values['round15-jobs.json']['jobs']]}
(root / 'remote-summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps(summary, ensure_ascii=False, indent=2))

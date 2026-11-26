"""仅等待精确提交的 Cloudleaf push CI 并保存实际任务状态。"""
from datetime import UTC, datetime
import json
from pathlib import Path
import subprocess
import sys
import time
import urllib.error
import urllib.request

SHA = sys.argv[1]
ROOT = Path(__file__).resolve().parent
OUT = ROOT / 'round23-ci/final-watch'
OUT.mkdir(parents=True, exist_ok=False)
API = 'https://api.github.com/repos/WanFengQC/Cloudleaf'
credential = subprocess.run(['git', '-c', 'credential.interactive=never', 'credential', 'fill'],
                            input=b'protocol=https\nhost=github.com\n\n', capture_output=True, timeout=30, check=True)
token = dict(line.split('=', 1) for line in credential.stdout.decode().splitlines() if '=' in line)['password']
headers = {'Accept': 'application/vnd.github+json', 'Authorization': 'Bearer ' + token,
           'User-Agent': 'Cloudleaf-Round23-exact-push', 'Cache-Control': 'no-cache'}

def get(path, filename):
    stamp = datetime.now(UTC).strftime('%Y%m%dT%H%M%S')
    link = API + path + '&_r22=' + stamp
    with urllib.request.urlopen(urllib.request.Request(link, headers=headers), timeout=25) as response:
        raw = response.read()
    (OUT / (stamp + '-' + filename)).write_bytes(raw)
    return json.loads(raw)

limit = time.monotonic() + 54 * 60
last = None
run = None
while time.monotonic() < limit:
    try:
        if run is None:
            records = get(f'/actions/runs?head_sha={SHA}&event=push&per_page=10', 'runs.json')
            matches = [entry for entry in records['workflow_runs']
                       if entry['head_sha'] == SHA and entry['event'] == 'push'
                       and entry['head_branch'] == 'codex/integration-round21' and entry['name'] == 'Core CI']
            assert len(matches) <= 1, '发现多个精确运行，需检查 workflow 身份'
            if matches:
                run = matches[0]
                (OUT / 'run-id.txt').write_text(str(run['id']) + '\n', encoding='utf-8')
                print(f"push {run['id']} {run['head_sha']} {run['html_url']}", flush=True)
        if run is not None:
            jobs = get(f"/actions/runs/{run['id']}/jobs?per_page=100", 'jobs.json')['jobs']
            assert all(job['head_sha'] == SHA for job in jobs)
            status = sorted((job['name'], job['status'], job['conclusion'],
                             [step['name'] for step in job['steps'] if step['status'] == 'in_progress'])
                            for job in jobs)
            if status != last:
                print(datetime.now(UTC).isoformat(), json.dumps(status), flush=True)
                last = status
            if len(jobs) == 2 and all(job['status'] == 'completed' for job in jobs):
                sys.exit(0 if all(job['conclusion'] == 'success' for job in jobs) else 1)
    except (OSError, TimeoutError) as error:
        print('GitHub read temporarily unavailable', type(error).__name__, flush=True)
    time.sleep(27)
sys.exit(2)


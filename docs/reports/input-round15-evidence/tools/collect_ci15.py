"""为R15新提交保存一次精确push CI快照；旧记录不覆盖。"""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import sys
import subprocess
import urllib.request

credential_result = subprocess.run(['git', '-c', 'credential.interactive=never', 'credential', 'fill'],
    input=b'protocol=https\nhost=github.com\n\n', stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
if credential_result.returncode:
    raise RuntimeError('已有Git凭据不可用；未请求远程修改或保存凭据内容')
credential = dict(line.split('=', 1) for line in credential_result.stdout.decode('utf-8').splitlines() if '=' in line)
authorization = 'Bearer ' + credential['password']

label, commit, run_id = sys.argv[1:]
run_id = int(run_id)
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / '.local/round15-ci' / label
OUT.mkdir(parents=True, exist_ok=False)
API = 'https://api.github.com/repos/WanFengQC/Cloudleaf'
record = {'checked_at_utc': datetime.now(timezone.utc).isoformat(), 'head_sha': commit, 'run_id': run_id,
          'authentication': 'Existing Git credential, process memory only; connector lists PR events only; exact push metadata uses official REST', 'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'HTTP_responses': [], 'verified_pass': False}


def fetch(suffix, name):
    url = API + suffix
    request = urllib.request.Request(url, headers={'Accept': 'application/vnd.github+json', 'User-Agent': 'Cloudleaf-Round15-exact-push-CI', 'Authorization': authorization})
    with urllib.request.urlopen(request, timeout=45) as response:
        raw = response.read()
        record['HTTP_responses'].append({'url': url, 'status': response.status, 'file': name, 'bytes': len(raw),
                                         'sha256': hashlib.sha256(raw).hexdigest(), 'headers': dict(response.headers)})
    (OUT / name).write_bytes(raw)
    return json.loads(raw)


try:
    runs = fetch(f'/actions/runs?head_sha={commit}&event=push&per_page=10', 'runs.json')
    run = fetch(f'/actions/runs/{run_id}', 'run.json')
    jobs = fetch(f'/actions/runs/{run_id}/jobs?per_page=100', 'jobs.json')['jobs']
    artifacts = fetch(f'/actions/runs/{run_id}/artifacts?per_page=100', 'artifacts.json')['artifacts']
    assert any(value['id'] == run_id and value['head_sha'] == commit and value['event'] == 'push' for value in runs['workflow_runs'])
    assert run['head_sha'] == commit and run['event'] == 'push'
    record.update({'url': run['html_url'], 'event': run['event'], 'status': run['status'], 'conclusion': run['conclusion']})
    record['jobs'] = [{'id': value['id'], 'name': value['name'], 'head_sha': value['head_sha'], 'status': value['status'],
                       'conclusion': value['conclusion'], 'steps': len(value['steps']),
                       'steps_ok': all(step['conclusion'] in ('success', 'skipped') for step in value['steps'])} for value in jobs]
    record['artifacts'] = [{'id': value['id'], 'name': value['name'], 'head_sha': value['workflow_run']['head_sha'],
                            'expired': value['expired'], 'digest': value.get('digest')} for value in artifacts]
    jobs_ok = len(jobs) == 2 and {value['name'] for value in jobs} == {'core (windows-latest)', 'core (ubuntu-latest)'}
    jobs_ok = jobs_ok and all(value['head_sha'] == commit and value['status'] == 'completed' and value['conclusion'] == 'success'
                              and all(step['conclusion'] in ('success', 'skipped') for step in value['steps']) for value in jobs)
    artifacts_ok = len(artifacts) == 3 and {value['name'] for value in artifacts} == {'core-performance-windows-latest', 'core-performance-ubuntu-latest', 'input-round15-windows'}
    artifacts_ok = artifacts_ok and all(not value['expired'] and value['workflow_run']['head_sha'] == commit
                                        and value.get('digest', '').startswith('sha256:') for value in artifacts)
    record['verified_pass'] = run['status'] == 'completed' and run['conclusion'] == 'success' and jobs_ok and artifacts_ok
except Exception as error:
    record['error'] = {'type': type(error).__name__, 'message': str(error)}
    raise
finally:
    (OUT / 'summary.json').write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({name: value for name, value in record.items() if name != 'HTTP_responses'}, ensure_ascii=False, indent=2))

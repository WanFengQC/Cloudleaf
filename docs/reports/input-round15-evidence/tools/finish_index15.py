from pathlib import Path
import hashlib
import json
import shutil
root = Path(__file__).resolve().parents[1]
out = root / 'docs/reports/input-round15-evidence'
old = json.loads((out / 'index.json').read_text(encoding='utf-8'))
for row in old['files']:
    assert hashlib.sha256((out / row['path']).read_bytes()).hexdigest() == row['sha256']
shutil.copytree(root / '.local/round15-runs/preservation-final', out / 'runs/preservation-final')
shutil.copyfile(root / '.local/round15-prs.json', out / 'PRs.json')
shutil.copyfile(Path(__file__), out / 'tools/finish_index15.py')
files = [{'path': str(path.relative_to(out)).replace('\\', '/'), 'bytes': path.stat().st_size,
    'sha256': hashlib.sha256(path.read_bytes()).hexdigest()} for path in sorted(out.rglob('*'))
    if path.is_file() and path != out / 'index.json']
(out / 'index.json').write_text(json.dumps({'files': files, 'index_excludes_itself': True,
    'fixed_runtime_or_full_research_model_or_binaries_included': False, 'authored_mechanical_fixtures_included': True,
    'final_exact_CI_receipt': '.local/round15-delivery-receipt.json'}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({'files': len(files), 'bytes': sum(row['bytes'] for row in files)}))

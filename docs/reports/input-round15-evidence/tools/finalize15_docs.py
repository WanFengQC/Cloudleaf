"""追加来源/历史检查和派生索引，不改任何已有原始日志。"""
from pathlib import Path
import hashlib
import json
import shutil
root = Path(__file__).resolve().parents[1]
out = root / 'docs/reports/input-round15-evidence'
index = json.loads((out / 'index.json').read_text(encoding='utf-8'))
for row in index['files']:
    assert hashlib.sha256((out / row['path']).read_bytes()).hexdigest() == row['sha256']
shutil.copyfile(root / '.local/round15-preservation.json', out / 'preservation.json')
shutil.copyfile(root / '.local/preserve15.py', out / 'tools/preserve15.py')
shutil.copyfile(root / '.local/finalize15_docs.py', out / 'tools/finalize15_docs.py')
(out / 'language/CC-CEDICT-NOTICE.md').write_text(
    '此研究sidecar引用固定CC-CEDICT词典stable entry ID；Google编纂CC BY3.0不解除词典CC-BY-SA4.0范围与署名/修改/共享义务。\n'
    '原字典数据、频率unknown与来源批准均未更改，完整派生TSV不入Git或CI，只作显式本地研究。\n'
    '来源、原文许可及固定镜像证据见data/sources/locks/cccedict-20260604-mirror7019e1c.json和data/licenses/CC-BY-SA-4.0.txt。\n',
    encoding='utf-8', newline='\n')
files = [{'path': str(path.relative_to(out)).replace('\\', '/'), 'bytes': path.stat().st_size,
    'sha256': hashlib.sha256(path.read_bytes()).hexdigest()} for path in sorted(out.rglob('*')) if path.is_file() and path.name != 'index.json']
(out / 'index.json').write_text(json.dumps({'files': files, 'index_excludes_itself': True,
    'fixed_runtime_or_full_research_model_or_binaries_included': False, 'authored_mechanical_fixtures_included': True,
    'final_exact_CI_receipt': '.local/round15-delivery-receipt.json'}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({'files': len(files), 'bytes': sum(row['bytes'] for row in files), 'original_index_files_preserved': len(index['files'])}))

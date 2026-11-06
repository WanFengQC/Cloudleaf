"""按原始字节归档本轮输入证据；不覆盖历史，不包含大词典/研究模型/二进制。"""
from pathlib import Path
import hashlib
import json
import re
import shutil
import subprocess
root = Path(__file__).resolve().parents[1]
out = root / 'docs/reports/input-round15-evidence'
out.mkdir(exist_ok=False)

def copy(source, relative):
    destination = out / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)

def tree(source, relative):
    source = Path(source)
    for path in source.rglob('*'):
        if not path.is_file() or any(part in ('target', '__pycache__', 'baseline-target', 'optimized-target') for part in path.parts):
            continue
        if path.suffix.lower() in ('.exe', '.dll', '.rlib', '.rmeta', '.zip', '.gz', '.bin', '.safetensors') or path.name == 'signal.tsv':
            continue
        copy(path, Path(relative) / path.relative_to(source))

for directory in (root / '.local/round15-runs').iterdir():
    tree(directory, Path('runs') / directory.name)
for path in (root / '.local').iterdir():
    if path.is_dir() and path.name.startswith(('round15-final-', 'round15-extra-', 'round15-trial-')):
        tree(path, Path('traces') / path.name)
for name in ['round15-paired-traces', 'round15-trainer']:
    tree(root / '.local' / name, name)
for name in ['round15-full-results.json', 'round15-final-dependency-audit.json', 'round15-lexicon-receipt.json', 'round15-performance.json']:
    copy(root / '.local' / name, name)
tree(root / '.local/round15-algorithm-v1', 'prototype-algorithm-v1')
for name in ['run15.py', 'full15.py', 'trace15.py', 'trace15_extra.py', 'paired_traces15.py',
             'prepare_r15_baseline.py', 'verify_trainer15.py', 'prepare_extra15.py', 'collect_ci15.py', 'download_ci15.py', 'archive15.py']:
    copy(root / '.local' / name, Path('tools') / name)
for name in ['report.json', 'LICENSE.txt', 'NOTICE.txt']:
    copy(root / '.local/round15-language' / name, Path('language') / name)
text = (root / '.local/round15-language/signal.tsv').read_text(encoding='utf-8')
(out / 'language/model-header.tsv').write_text('\n'.join(text.splitlines()[:10]) + '\n', encoding='utf-8', newline='\n')
for source, target in [
    ('r15-search', 'recall-diagnosis'), ('r15-model-review', 'model-review'),
    ('r15-final-review', 'final-lexical-differential'), ('r15-phase-final', 'phase-comparison'),
    ('r15-data/verification-final', 'data-verification'), ('r15-perf/finish-20261104', 'optimization-before-after')]:
    tree(Path('C:/Users/EDY/.codex/tmp') / source, target)
full = json.loads((root / '.local/round15-full-results.json').read_text(encoding='utf-8'))
assert len(full) == 13 and all(row['returncode'] == 0 for row in full)
implementation = '994704c8ce9b5780b8cc53a32ca5c1f731094b30'
assert all(row['head'] == implementation for row in full)
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip() == implementation
rust = (root / '.local/round15-runs/final-04/stdout.txt').read_text(encoding='utf-8')
python = (root / '.local/round15-runs/final-08/stderr.txt').read_text(encoding='utf-8')
paired = json.loads((root / '.local/round15-paired-traces/summary.json').read_text(encoding='utf-8'))
model = json.loads((root / '.local/round15-language/report.json').read_text(encoding='utf-8'))
summary = {'implementation_head': implementation, 'status': 'partial', 'full_commands': len(full),
    'rust_passed': sum(map(int, re.findall(r'test result: ok\. (\d+) passed', rust))),
    'rust_ignored': sum(map(int, re.findall(r'; (\d+) ignored', rust))),
    'python_passed': int(re.search(r'Ran (\d+) tests', python).group(1)), 'doc_test_examples': 0,
    'model': {key: model[key] for key in ['signal_sha256', 'signal_bytes', 'usage', 'projected_entry_ids', 'missing_entry_ids', 'source_status_unchanged']},
    'actual_input_event_latency_ns': paired['percentiles_ns'],
    'original_structural_behavior_unchanged': paired['optimized_structural_output_and_statistics_identical'],
    'formal_language_metrics': {'top1': None, 'top5': None, 'mrr': None},
    'default_input_backend': 'legacy unchanged; model opt-in only',
    'Bigram_and_Context': 'version-bound interfaces and mechanical tests implemented; real transition corpus absent; inactive for real-data runs',
    'incremental': 'not implemented; full recomputation retained',
    'cancellation': 'search/lattice cooperative API and actual boundary tests; not wired to synchronous Host key handling; no deadline or async scheduling',
    'TSF': False, 'long_soak': False, 'CL010': 'closed',
    'binary_SHA256': {name: hashlib.sha256((root / '.local/build/release' / name).read_bytes()).hexdigest()
        for name in ['cloudleaf-cli.exe', 'cloudleaf-ime.exe', 'cloudleaf_windows_ime.dll', 'examples/input_trace.exe', 'examples/train_language.exe']}}
(out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
(out / 'changed-files.tsv').write_text(subprocess.check_output(['git', 'diff', '--name-status', '75e0ace', implementation], cwd=root).decode(), encoding='utf-8', newline='\n')
table = ['# 固定真实词库Top10：研发对照，不是独立gold', '', '同171事件、Top10、PositionDag与固定预算。完整来源/路径/分数见paired-traces summary及逐次trace。', '']
for key in ['nihao', 'zhongguo', 'woaini', 'weather', 'target', 'complex', 'ambiguous', 'separated']:
    before = paired['cases']['original'][key]
    after = paired['cases']['lexical'][key]
    table.extend([f'## {key}: `{before["raw"]}`', '', '| 次序 | R14原结构 | R15研究词汇 |', '|---|---|---|'])
    for index in range(max(len(before['candidates']), len(after['candidates']))):
        old = before['candidates'][index]['text'] if index < len(before['candidates']) else ''
        new = after['candidates'][index]['text'] if index < len(after['candidates']) else ''
        table.append(f'| {index + 1} | {old} | {new} |')
    table.append('')
(out / 'candidate-comparison.md').write_text('\n'.join(table) + '\n', encoding='utf-8', newline='\n')
files = [{'path': str(path.relative_to(out)).replace('\\', '/'), 'bytes': path.stat().st_size,
    'sha256': hashlib.sha256(path.read_bytes()).hexdigest()} for path in sorted(out.rglob('*')) if path.is_file()]
(out / 'index.json').write_text(json.dumps({'files': files, 'index_excludes_itself': True,
    'runtime_and_research_model_and_binaries_included': False,
    'final_exact_CI_receipt': '.local/round15-delivery-receipt.json'}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({'files': len(files), 'bytes': sum(row['bytes'] for row in files), 'rust': summary['rust_passed'], 'python': summary['python_passed']}))

"""只在root通知测量窗口后显式运行；不与编译或输入轨迹采样并行。"""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import statistics
import subprocess
import sys
import time

if sys.argv[1:] != ['--measurement-window-approved']:
    raise SystemExit('尚未进入指定测量窗口；未启动探针。')

tool = Path(__file__).resolve().parent
root = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out = tool / 'measurements'
out.mkdir(exist_ok=False)
compile_record = json.loads((tool / 'compile.json').read_text(encoding='utf-8'))
assert compile_record['returncode'] == 0 and compile_record['no_measurement_executed']
exe = tool / 'target/release/r15-phase-final.exe'
assert hashlib.sha256(exe.read_bytes()).hexdigest() == compile_record['executable_sha256']
receipt = json.loads((root / '.local/round14-lexicon-receipt.json').read_text(encoding='utf-8'))
model_report = json.loads((root / '.local/round15-language/report.json').read_text(encoding='utf-8'))
runtime = Path(receipt['runtime']['path'])
model = root / '.local/round15-language/signal.tsv'
assert hashlib.sha256(runtime.read_bytes()).hexdigest() == receipt['runtime']['sha256']
assert hashlib.sha256(model.read_bytes()).hexdigest() == model_report['signal_sha256']
assert model_report['inputs']['lexicon']['sha256'] == receipt['runtime']['sha256']
cases = out / 'cases.tsv'
cases.write_bytes((root / 'docs/reports/input-round14-evidence/round14-search-comparison/cases.tsv').read_bytes())
case_rows = cases.read_text(encoding='utf-8').splitlines()[2:]
assert len(case_rows) == 12
source_files = sorted((root / 'crates/cloudleaf-search/src').glob('*.rs'))
source_before = {str(path.relative_to(root)).replace('\\', '/'): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_files}
for name, sha in compile_record['source_before'].items():
    assert source_before[name] == sha

commands = []
summaries = []
started = time.perf_counter()
for method in ['reference', 'exact', 'position', 'beam']:
    for ranker in ['structural', 'lexical']:
        label = method + '-' + ranker
        command = [str(exe), str(runtime), str(cases), method, ranker, str(model), receipt['runtime']['sha256'],
                   '8', '3', '1', '16384', '200000', '50000', '10']
        begin = time.perf_counter()
        result = subprocess.run(command, cwd=tool, capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (label + '.json')).write_bytes(result.stdout)
        (out / (label + '.stderr.txt')).write_bytes(result.stderr)
        row = {'method': method, 'ranker': ranker, 'command': command, 'returncode': result.returncode,
               'seconds': time.perf_counter()-begin, 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
               'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
        commands.append(row)
        try:
            record = json.loads(result.stdout)
            for case in record['cases']:
                samples = case['samples']
                actual = case.get('result', {})
                summaries.append({'method': method, 'ranker': ranker, 'id': case['id'], 'input': case['input'],
                    'plain_ns': [sample['plain_ns'] for sample in samples],
                    'profile_outer_ns': [sample['profile_outer_ns'] for sample in samples],
                    'all_equal': all(sample['equal'] for sample in samples),
                    'plain_median_ns': statistics.median(sample['plain_ns'] for sample in samples),
                    'status': actual.get('status'), 'guaranteed': actual.get('guaranteed'),
                    'statistics': actual.get('statistics'),
                    'top1': actual.get('candidates', [{}])[0].get('text') if actual.get('candidates') else None,
                    'phase_median_ns': {phase: statistics.median(sample['profile']['phases'][phase]['ns'] for sample in samples)
                                        for phase in samples[0]['profile']['phases'] if samples[0]['profile']['phases'][phase] is not None}})
        except Exception as error:
            row['parse_or_summary_error'] = {'type': type(error).__name__, 'message': str(error)}
        print(json.dumps({'method': method, 'ranker': ranker, 'returncode': result.returncode, 'seconds': row['seconds']}), flush=True)

source_after = {str(path.relative_to(root)).replace('\\', '/'): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_files}
all_valid = len(summaries) == 96 and all(row['returncode'] == 0 and 'parse_or_summary_error' not in row for row in commands)
all_valid = all_valid and all(row['all_equal'] and len(row['plain_ns']) == 3 for row in summaries) and source_before == source_after
record = {'schema_version': 1, 'at_utc': datetime.now(timezone.utc).isoformat(),
          'implementation_head': compile_record['head'],
          'observed_head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip(),
          'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'probe_sha256': compile_record['probe_sha256'], 'executable_sha256': compile_record['executable_sha256'],
          'runtime': receipt['runtime'], 'model': {'path': str(model), 'sha256': model_report['signal_sha256'], 'usage': 'research_only'},
          'cases_sha256': hashlib.sha256(cases.read_bytes()).hexdigest(),
          'source_before': source_before, 'source_after': source_after, 'source_unchanged': source_before == source_after,
          'configuration': {'output_k': 10, 'beam_width': 8, 'max_edges': 16384, 'max_expansions': 200000, 'max_resident_paths': 50000},
          'iterations': 3, 'warmups': 1, 'paired_measurements': sum(len(row['plain_ns']) for row in summaries),
          'seconds': time.perf_counter()-started, 'all_pairs_equal': all_valid,
          'phase_relation': 'Traversal includes nested Scoring and BucketSortDedup. Do not sum all phases.',
          'resource_boundary': 'Deterministic path/edge counts, not measured per-algorithm allocated byte memory.',
          'formal_language_metrics': None, 'commands': commands, 'cases': summaries}
with (out / 'result.json').open('x', encoding='utf-8', newline='\n') as file:
    json.dump(record, file, ensure_ascii=False, indent=2);file.write('\n')
print(json.dumps({'pairs': record['paired_measurements'], 'all_pairs_equal': all_valid,
                  'seconds': record['seconds'], 'source_unchanged': record['source_unchanged']}))
raise SystemExit(not all_valid)

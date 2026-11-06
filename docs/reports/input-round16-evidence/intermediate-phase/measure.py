"""真实模型阶段配对；所有失败、预算、剪枝及拒绝结果保留。"""
from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import statistics
import subprocess
import sys
import time

root = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
tool = Path(__file__).parent
label, compile_label, *flags = sys.argv[1:]
assert flags in ([], ['--controlled-window'])
controlled = bool(flags)
out = tool / label
out.mkdir(exist_ok=False)
compile_record = json.loads((tool / compile_label / 'compile.json').read_text(encoding='utf-8'))
assert compile_record['returncode'] == 0 and compile_record['source_unchanged']
exe = tool / 'target/release/r16-phase-profile.exe'
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
assert sha(exe) == compile_record['executable_sha256']
runtime = Path('C:/Users/EDY/.codex/worktrees/cl009-data/Cloudleaf/.local/round5-clean-6d3d4e1/release-a/runtime.tsv')
runtime_sha = '03891e9e7540fca778d02fcd5c468825a9c32b96dabb8dac6de54b21b72f1431'
assert sha(runtime) == runtime_sha
lexical = root / '.local/round15-language/signal.tsv'
character = root / '.local/round16-language/character.tsv'
assert sha(lexical) == '36dc4d9057bc70e3494fe397da09e8ab6f187c435bd735b66a7734f9d414fce5'
assert sha(character) == '5f28834165fcf83b22458e701b3608b403cb9bf7a9be8ba3c686fd158c53d6b9'
cases = tool / 'cases.tsv'
assert len(cases.read_text(encoding='utf-8').splitlines()[2:]) == 12
def source():
    return {name: sha(root / name) for name in compile_record['source_before']}
before = source()
record = {'at_utc': datetime.now(timezone.utc).isoformat(), 'head_at_measurement_start': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip(), 'compile_record': compile_record, 'runtime': {'path': str(runtime), 'sha256': runtime_sha}, 'models': {'lexical': {'path': str(lexical), 'sha256': sha(lexical)}, 'character': {'path': str(character), 'sha256': sha(character)}}, 'probe_sha256': sha(tool / 'src/main.rs'), 'tool_sha256': sha(Path(__file__)), 'cases_sha256': sha(cases), 'configuration': {'output_k': 10, 'beam_width': 64, 'max_edges': 16384, 'max_expansions': 200000, 'max_resident_paths': 50000}, 'warmup_per_case_per_entry': 1, 'iterations_per_case': 3, 'phase_relation': 'Scoring and BucketSortDedup are nested inside Traversal. QueryTotal includes all top-level phases. Do not sum nested phases.', 'resource_boundary': 'Deterministic graph/path counts; not per-algorithm allocated byte memory. Query stages exclude disk/model loading, host/platform and input frame cloning.', 'formal_language_metrics': {'top1': None, 'top5': None, 'mrr': None}, 'source_before': before, 'commands': [], 'cases': [], 'relationship_violations': [], 'background_load': 'Exploratory current-source measurements; other agent may compile concurrently. Final source measurements require a separate label/window.'}
record['controlled_window_requested'] = controlled
if controlled:
    record['background_load'] = 'Root designated measurement window, no known root compilation or trace concurrent; OS cache not cleared and unrelated system load not guaranteed idle.'
    assert record['head_at_measurement_start'] == compile_record['head']
started = time.perf_counter()
for method in ('reference', 'exact', 'position', 'beam'):
    for ranker in ('structural', 'lexical', 'character'):
        name = method + '-' + ranker
        model = character if ranker == 'character' else lexical
        command = [str(exe), str(runtime), str(cases), method, ranker, str(model), runtime_sha, '64', '3', '1', '16384', '200000', '50000', '10']
        start = time.perf_counter()
        result = subprocess.run(command, cwd=tool, capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
        (out / (name + '.json')).write_bytes(result.stdout)
        (out / (name + '.stderr.txt')).write_bytes(result.stderr)
        row = {'method': method, 'ranker': ranker, 'command': command, 'returncode': result.returncode, 'seconds': time.perf_counter() - start, 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
        record['commands'].append(row)
        try:
            raw = json.loads(result.stdout)
            assert len(raw['cases']) == 12
            for case in raw['cases']:
                samples = case['samples']
                actual = case.get('result', {})
                record['cases'].append({'method': method, 'ranker': ranker, 'id': case['id'], 'input': case['input'], 'all_equal': all(s['equal'] for s in samples), 'n': len(samples), 'plain_ns': [s['plain_ns'] for s in samples], 'profile_outer_ns': [s['profile_outer_ns'] for s in samples], 'plain_median_ns': statistics.median(s['plain_ns'] for s in samples), 'status': actual.get('status'), 'error_kind': actual.get('error_kind'), 'guaranteed': actual.get('guaranteed'), 'statistics': actual.get('statistics'), 'top1': actual.get('candidates', [{}])[0].get('text') if actual.get('candidates') else None, 'phase_median_ns': {p: statistics.median(s['profile']['phases'][p]['ns'] for s in samples) for p in samples[0]['profile']['phases'] if samples[0]['profile']['phases'][p] is not None}})
                for i, sample in enumerate(samples):
                    phases = sample['profile']['phases']
                    values = {p: m['ns'] if m is not None else 0 for p, m in phases.items()}
                    top_level = sum(values[p] for p in ('normalize', 'lattice', 'search_validate', 'traversal', 'final_sort_dedup', 'context_rerank', 'materialize'))
                    checks = {'clock_reads': sample['profile']['clock_reads'] == 2 * sum(m['calls'] for m in phases.values() if m is not None), 'outer_ge_total': sample['profile_outer_ns'] >= values['query_total'], 'total_ge_top_level_sum': values['query_total'] >= top_level, 'traversal_ge_nested_sum': values['traversal'] >= values['scoring'] + values['bucket_sort_dedup']}
                    for check, passed in checks.items():
                        if not passed:
                            record['relationship_violations'].append({'case': name + ':' + case['id'], 'sample': i, 'check': check, 'values': values})
        except Exception as error:
            row['parse_error'] = {'type': type(error).__name__, 'message': str(error)}
        print(json.dumps({'method': method, 'ranker': ranker, 'returncode': result.returncode, 'seconds': row['seconds']}), flush=True)
record['source_after'] = source()
record['source_unchanged_during_measurement'] = record['source_before'] == record['source_after']
record['source_matches_compile_at_start'] = record['source_before'] == compile_record['source_after']
record['head_at_measurement_end'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip()
record['seconds'] = time.perf_counter() - started
record['paired_measurements'] = sum(row['n'] for row in record['cases'])
record['all_pairs_equal'] = len(record['cases']) == 144 and all(row['all_equal'] and row['n'] == 3 for row in record['cases']) and all(row['returncode'] == 0 and 'parse_error' not in row for row in record['commands'])
record['phase_relationships_pass'] = not record['relationship_violations']
record['not_final_version_claim'] = not (record['source_unchanged_during_measurement'] and record['source_matches_compile_at_start'])
(out / 'result.json').write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({key: record[key] for key in ('paired_measurements', 'all_pairs_equal', 'phase_relationships_pass', 'source_unchanged_during_measurement', 'source_matches_compile_at_start', 'not_final_version_claim', 'seconds')}, ensure_ascii=False), flush=True)
raise SystemExit(not record['all_pairs_equal'] or not record['phase_relationships_pass'])

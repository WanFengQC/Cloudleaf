"""归档离线缓存试验，所有原始输出保留为可校验字节。"""
from pathlib import Path
import gzip
import hashlib
import json
import math
import statistics

root = Path(__file__).resolve().parent
parent = root.parent
out = parent / 'r20-cache-delivery'
base = '4d42363091361552ad525f47e3c6b1d581ae3a4c'
models = ('lexical', 'addone', 'witten')
sizes = (-1, 0, 4, 512, 4096)
schemes = ('full', 'abbreviated', 'xiaohe', 'fuzzy-nl', 'correction')
sha = lambda raw: hashlib.sha256(raw).hexdigest()

def load(path):
    return json.loads(path.read_text(encoding='utf-8'))

def write(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')

def percentiles(values):
    ordered = sorted(values)
    p = lambda q: ordered[math.ceil(len(ordered) * q) - 1]
    return {'n': len(ordered), 'p50': p(.5), 'p95': p(.95), 'p99': p(.99), 'max': ordered[-1]}

current = load(root / 'cache-measurements-current/summary.json')
events = load(root / 'cache-events-current/summary.json')
assert current['runs'] == 30 and current['paired_complete_objects'] == 4185
assert events['all_equal'] and events['events_compared'] == 2565
metrics = current['metrics']
comparisons = {}
for scheme in schemes:
    lattice_changes = []
    total_changes = []
    hits, lookups, evictions = 0, 0, 0
    for model in models:
        for size in sizes:
            off = metrics[f'{model}/{size}/off/{scheme}']
            on = metrics[f'{model}/{size}/on/{scheme}']
            lattice_changes.append(100 * (on['lattice_ns']['p95'] / off['lattice_ns']['p95'] - 1))
            total_changes.append(100 * (on['total_query_ns']['p95'] / off['total_query_ns']['p95'] - 1))
            stats = on['cache_statistics']
            hits += stats['hits']
            lookups += stats['lookups']
            evictions += stats['evictions']
    relevant = [r for r in events['pairs'] if r['scheme'] == scheme]
    event_changes = [100 * (r['on']['latency_ns']['p95'] / r['off']['latency_ns']['p95'] - 1) for r in relevant]
    comparisons[scheme] = {
        'lattice_p95_improved_groups': sum(v < 0 for v in lattice_changes),
        'median_lattice_p95_change_percent': statistics.median(lattice_changes),
        'total_query_p95_improved_groups': sum(v < 0 for v in total_changes),
        'median_total_query_p95_change_percent': statistics.median(total_changes),
        'cache_hits': hits, 'cache_lookups': lookups, 'cache_evictions': evictions,
        'cache_hit_rate': hits / lookups if lookups else None,
        'event_p95_changes_percent': event_changes,
        'event_p95_improved_models': sum(v < 0 for v in event_changes),
    }

sources = {}
for area in ('crates/cloudleaf-search/src', 'crates/cloudleaf-pinyin/src', 'crates/cloudleaf-input/src'):
    for path in sorted((root / area).rglob('*.rs')):
        sources[path.relative_to(root).as_posix()] = sha(path.read_bytes())
for name in ('crates/cloudleaf-input/examples/transition_cache_probe.rs',
             'crates/cloudleaf-input/examples/incremental_trace.rs', 'Cargo.lock'):
    sources[name] = sha((root / name).read_bytes())
data = {'baseline': base, 'baseline_character_search': True,
        'query_runs': current['runs'], 'query_pairs': current['paired_complete_objects'],
        'query_budget_failures': {mode: sum(v['budget_failures'] for k, v in metrics.items() if f'/{mode}/' in k) for mode in ('off', 'on')},
        'event_pairs': events['events_compared'], 'event_observations': events['observations'],
        'scheme_comparisons': comparisons, 'source_sha256': sources,
        'scope': '独立实验工作副本。生产实现保留显式opt-in；临时版使用RefCell和基线字符搜索，性能不可视作生产最终二进制。'}

out.mkdir(exist_ok=False)
manifest = {}
def add(source, key):
    raw = source.read_bytes()
    encoded = gzip.compress(raw, mtime=0) if len(raw) > 100_000 else raw
    target = out / (key + ('.gz' if encoded != raw else ''))
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(encoded)
    manifest[target.relative_to(out).as_posix()] = {
        'bytes': len(encoded), 'sha256': sha(encoded), 'original_bytes': len(raw), 'original_sha256': sha(raw)}

for folder in ('cache-measurements', 'cache-measurements-paired', 'cache-measurements-current', 'cache-events-current'):
    for path in sorted((root / folder).rglob('*')):
        if path.is_file():
            add(path, path.relative_to(root).as_posix())
for path in [*root.glob('*.log'), *root.glob('*.py'), root / 'design.md']:
    add(path, path.name)
for name in ('crates/cloudleaf-search/src/transition_cache.rs', 'crates/cloudleaf-search/src/schemes.rs',
             'crates/cloudleaf-search/src/lib.rs', 'crates/cloudleaf-input/src/sentence.rs',
             'crates/cloudleaf-input/src/lib.rs', 'crates/cloudleaf-input/tests/incremental_cache.rs',
             'crates/cloudleaf-search/tests/transition_cache.rs',
             'crates/cloudleaf-input/examples/transition_cache_probe.rs',
             'crates/cloudleaf-input/examples/incremental_trace.rs'):
    add(root / name, f'experimental-source/{name}')
write(out / 'summary.json', data)
lines = [
    '# Round 20 Trie 音节转移缓存实验', '',
    '建议输入默认关闭，保留显式启用接口。命中省去一部分 Trie 逐字节遍历，但在固定查询组中没有证实简拼建格收益。实验构建每引擎最多 4,096 个槽，容量约 98,424 字节。', '',
    '独立副本基于 Round 19 字符搜索，使用 RefCell 串接缓存。主工作区使用显式启用、可变借用和新的字符搜索路径。这次二进制时延只用于缓存取舍。', '',
    '固定 123,544 条词典、三种语言模型与五种用户层规模，30 个进程各重放 279 次最终查询，合计 8,370 次。相同输入的 4,185 对完整 SearchResult 和候选边来源无差异，预算失败每模式各 30 次。用户层使用合成词条且不含个人数据。', '',
    '五种方案、三种模型逐键各执行两个进程，分别关闭和开启缓存。每个进程 171 事件，2,565 对 EventResult 及 SearchDiagnostic 全部相等，只有计时和缓存计数独立记录。包含 5,130 次实际事件观测。提交确认来自显式事件重放。', '',
    '| 方案 | 最终查询组中建格 P95 降低 | 建格 P95 中位变化 | 总查询 P95 中位变化 | 实验命中率 | 逐键 P95 降低的模型 |',
    '| --- | ---: | ---: | ---: | ---: | ---: |',
]
for scheme, row in comparisons.items():
    rate = row['cache_hit_rate']
    rate_text = '无查表' if rate is None else f'{rate:.2%}'
    lines.append(f"| {scheme} | {row['lattice_p95_improved_groups']}/15 | {row['median_lattice_p95_change_percent']:+.1f}% | {row['median_total_query_p95_change_percent']:+.1f}% | {rate_text} | {row['event_p95_improved_models']}/3 |")
lines += ['', '全拼试验没有执行缓存查表，启停之间的时延变化提供了本机波动的参照。逐键 P95 跨模型也有波动，不能从单个模型的配对改善推定上线收益。不同模型和词库规模的 P50/P95/P99/max、状态数、完整来源、预算、淘汰和失效计数见各组原始证据。计数用于机械回归，真实 TSF 和中文独立质量仍待验收。初次中断与旧二进制复测的 stdout/stderr 另保留。', '']
(out / 'summary.md').write_text('\n'.join(lines), encoding='utf-8', newline='\n')
write(out / 'index.json', manifest)
print(json.dumps({'files': len(manifest), 'bytes': sum(row['bytes'] for row in manifest.values()), 'query_pairs': 4185, 'event_pairs': 2565, 'comparisons': comparisons}, ensure_ascii=False))

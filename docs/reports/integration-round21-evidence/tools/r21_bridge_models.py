"""实权重无窗口的显式翻译、确认和故障证据。"""

import argparse
import json
from pathlib import Path

from r21_evidence_common import LOCAL, ROOT, get_json, head, identity, now, percentile, put_json, run
from r21_input_traces import KNOWN, RUNTIME
from r21_real_models import DEFAULT_CATALOG, PINNED_CATALOG, approved_catalog, faults


def op(name, **values):
    return {'op': name, **values}


def steps():
    return [
        op('activate'), op('type', text='nihao'),
        op('candidate', index=0), op('wait'), op('confirm'), op('applied'),
        op('applied'),
        op('english', text='Cloudleaf keeps its original input until confirmation.'),
        op('request-english'), op('wait'), op('confirm'), op('not-applied'), op('retry'), op('applied'),
        op('english', text='The document preserves both sentences. The coffee weighs 12.5 kilograms.'),
        op('request-english'), op('wait'), op('confirm'), op('unknown'), op('retry'),
        op('reset-unknown', session=2), op('activate'),
        op('english', text='Focus changes before the user confirms.'),
        op('request-english'), op('wait'), op('blur'), op('confirm'),
    ]


def verify_trace(rows):
    # 对于研发宿主，只有显式 Applied 后可以向内存文档追加。
    assert len(rows) == len(steps())
    for index, (row, action) in enumerate(zip(rows, steps(), strict=True)):
        assert row['index'] == index and row['op'] == action['op'] and row['target'] == 'in-memory'
        assert not row['learning_selection'], row
    expected = {
        3: ('Ready', None), 4: ('CommitPending', None), 5: ('Completed', None),
        6: ('Completed', 'StaleReceipt'),
        9: ('Ready', None), 10: ('CommitPending', None), 11: ('RetryReady', None),
        12: ('CommitPending', None), 13: ('Completed', None),
        16: ('Ready', None), 17: ('CommitPending', None), 18: ('CommitUnknown', None),
        19: ('CommitUnknown', 'OutcomeUnknown'), 20: ('Idle', None),
        24: ('Ready', None), 25: ('Stale', None), 26: ('Stale', 'StaleConfirmation'),
    }
    for index, (state, error) in expected.items():
        row = rows[index]
        assert row['state'] == state and row['error'] == error, (index, state, error, row)
    for index in (3, 9, 16, 24):
        row = rows[index]
        assert row['quality'] == 'Generated' and row['model_identity'] == PINNED_CATALOG
        assert row['original'].strip() and row['translation'].strip()
    assert rows[3]['original'] == '你好'
    assert rows[9]['original'] == steps()[7]['text']
    assert rows[11]['original'] == rows[9]['original']
    assert rows[10]['commit'] != rows[12]['commit']
    assert rows[16]['original'] == rows[18]['original']
    first = rows[3]['translation']
    second = first + rows[9]['translation']
    assert all(r['document'] == ('' if i < 5 else first if i < 13 else second)
               for i, r in enumerate(rows))
    return {
        'verified_states': {str(i): v[0] for i, v in expected.items()},
        'explicit_applied_count': 2, 'both_directions': True, 'selection_evidence_emitted': False,
        'loaded_model_identity': PINNED_CATALOG,
        'chinese_candidate': rows[3]['original'], 'chinese_to_english': rows[3]['translation'],
        'english_to_chinese': rows[9]['translation'],
        'numeric_and_units_example': {'original': rows[16]['original'], 'translation': rows[16]['translation']},
        'latency_ns': {k: percentile([r['elapsed_ns'] for r in rows if r['op'] == k])
                       for k in ('type', 'request-english', 'candidate', 'wait', 'confirm')},
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('release', type=Path)
    parser.add_argument('--label', default='round21-real-bridge')
    args = parser.parse_args()
    exe = args.release.resolve() / 'cloudleaf-bridge-headless.exe'
    out = LOCAL / args.label
    out.mkdir(exist_ok=False)
    catalog = DEFAULT_CATALOG
    material = approved_catalog(catalog)
    assert identity(RUNTIME)['sha256'] == KNOWN['runtime']
    report = {'head': head(), 'started_utc': now(), 'tool': identity(__file__),
              'binary': identity(exe), 'dictionary': identity(RUNTIME), 'model_catalog': material,
              'source': {p.relative_to(ROOT).as_posix(): identity(p)['sha256']
                         for p in (ROOT / 'crates/cloudleaf-bridge').rglob('*.rs')},
              'scope': 'actual local models on a background worker, synthetic in-memory receipts only',
              'formal_language_metrics': None, 'runs': []}
    put_json(out / 'summary.json', report)

    start = [op('activate'), op('type', text='nihao'), op('candidate', index=0)]
    failed = [*start, op('wait'), op('confirm'), op('type', text='a')]
    stale = [*start, op('type', text='a'), op('wait'), op('confirm')]
    cancelled = [*start, op('cancel'), op('wait'), op('confirm')]
    broken = faults(catalog, out)
    missing = out / 'not-installed' / 'catalog.json'
    for name, cat, sha, actions in (
        ('bridge-bidirectional', catalog, PINNED_CATALOG, steps()),
        ('bridge-stale', catalog, PINNED_CATALOG, stale),
        ('bridge-cancelled', catalog, PINNED_CATALOG, cancelled),
        ('bridge-missing', missing, PINNED_CATALOG, failed),
        ('bridge-damaged', broken, identity(broken)['sha256'], failed),
    ):
        payload = ''.join(json.dumps(a, ensure_ascii=False) + '\n' for a in actions).encode('utf-8')
        (out / f'{name}.jsonl').write_bytes(payload)
        record = run(out / name, [exe, RUNTIME, KNOWN['runtime'], cat, sha, '--show-text'],
                     payload=payload, deadline_seconds=180)
        report['runs'].append(record)
        rows = [json.loads(s) for s in (out / name / 'stdout.txt').read_text(encoding='utf-8').splitlines()]
        put_json(out / 'summary.json', report)
        if name == 'bridge-bidirectional':
            report['verified'] = verify_trace(rows)
        elif name in {'bridge-stale', 'bridge-cancelled'}:
            state = 'Stale' if name == 'bridge-stale' else 'Cancelled'
            assert rows[4]['state'] == state and rows[4]['original'] == '你好', rows[4]
            assert rows[5]['error'] == 'StaleConfirmation'
            assert all(not row['document'] and not row['learning_selection'] for row in rows)
            report.setdefault('cancel_and_invalidation', {})[name] = state
        else:
            assert rows[3]['state'] == 'Failed' and rows[3]['original'] == '你好', rows[3]
            assert rows[3]['translation_failure'] == ('ModelNotFound' if name == 'bridge-missing' else 'ModelLoadFailed')
            assert rows[4]['error'] == 'StaleConfirmation' and not rows[4]['document']
            assert rows[5]['preedit_bytes'] == 6 and rows[5]['error'] is None
            report.setdefault('faults', {})[name] = rows[3]['translation_failure']
    assert approved_catalog(catalog) == material
    report['catalog_unchanged'] = True
    report['finished_utc'] = now()
    put_json(out / 'summary.json', report)
    print(json.dumps(report['verified'], ensure_ascii=False))


if __name__ == '__main__':
    main()

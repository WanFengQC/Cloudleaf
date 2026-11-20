"""分列组件字节覆盖与公共包锁，不把同文件名当作功能或历史等价。"""
from pathlib import Path
import hashlib
import json
import subprocess
import tomllib

ROOT = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
OUT = Path(__file__).resolve().parent
summary = json.loads((OUT / 'summary.json').read_text(encoding='utf-8'))
refs = summary['refs']

def call(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)

def files(ref, prefix):
    return call('ls-tree', '-r', '--name-only', ref, '--', prefix).decode('utf-8').splitlines()

report = {'components': {}, 'dependencies': {}}
groups = {'search': ['crates/cloudleaf-search', 'scripts/evaluate_real_lexicon.py',
                     'scripts/check_search_resources.py', 'scripts/search_resource_contract.py'],
          'windows': ['adapters/windows', 'apps/windows-ime', 'scripts/check_windows_soak_admission.py',
                      'scripts/stress_windows_faults.py', 'scripts/stress_windows_owner.py', 'scripts/report_windows_soak.py'],
          'data': ['tools/data_pipeline', 'tests', 'data/sources', 'data/licenses'],
          'translation': ['crates/cloudleaf-translation', 'crates/cloudleaf-offline',
                          'apps/cloudleaf-translation-panel', 'vendor/offline', 'config/offline-models',
                          'scripts/audit_translation_dependencies.py']}
for name, prefixes in groups.items():
    ours = set(path for prefix in prefixes for path in files(refs['input'], prefix))
    theirs = set(path for prefix in prefixes for path in files(refs[name], prefix))
    same, different = [], []
    identities = {}
    for path in sorted(ours.intersection(theirs)):
        a = call('show', refs['input'] + ':' + path)
        b = call('show', refs[name] + ':' + path)
        (same if a == b else different).append(path)
        identities[path] = {'input_sha256': hashlib.sha256(a).hexdigest(),
                            'branch_sha256': hashlib.sha256(b).hexdigest()}
    report['components'][name] = {'prefixes': prefixes, 'same': same, 'different': different,
                                  'input_only': sorted(ours - theirs),
                                  'branch_only': sorted(theirs - ours), 'identities': identities}
input_lock = tomllib.loads(call('show', refs['input'] + ':Cargo.lock').decode('utf-8'))
def packages(lock):
    return {(row['name'], row['version'], row.get('source')): row for row in lock['package']}
a = packages(input_lock)
for name in ['search', 'data', 'windows', 'translation']:
    lock = tomllib.loads(call('show', refs[name] + ':Cargo.lock').decode('utf-8'))
    b = packages(lock)
    report['dependencies'][name] = {'input_package_records': len(input_lock['package']),
                                    'branch_package_records': len(lock['package']),
                                    'added_package_keys': sorted(set(b) - set(a)),
                                    'same_key_different_record': {str(key): {'input_dependencies': a[key].get('dependencies', []),
                                                                          'branch_dependencies': b[key].get('dependencies', [])}
                                                                 for key in a.keys() & b.keys() if a[key] != b[key]},
                                    'shared_names_version_sets': {
                                        package: {'input': sorted({key[1] for key in a if key[0] == package}),
                                                  'branch': sorted({key[1] for key in b if key[0] == package})}
                                        for package in {key[0] for key in a} & {key[0] for key in b}
                                        if {key[1] for key in a if key[0] == package} != {key[1] for key in b if key[0] == package}}}
(OUT / 'components.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'components': {n: {k: len(r[k]) for k in ['same', 'different', 'input_only', 'branch_only']} for n, r in report['components'].items()},
                   'dependencies': {n: {'input': r['input_package_records'], 'branch': r['branch_package_records'],
                                         'shared_version_sets': r['shared_names_version_sets'],
                                         'changed_same_key': len(r['same_key_different_record'])} for n, r in report['dependencies'].items()}}, ensure_ascii=False, indent=2))

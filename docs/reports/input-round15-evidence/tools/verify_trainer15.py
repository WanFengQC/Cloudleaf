"""实际离线训练器的原创机械数据往返与拒绝路径，不能当自然语言训练。"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
root = Path(__file__).resolve().parents[1]
out = root / '.local/round15-trainer'
out.mkdir(exist_ok=False)
words = '# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\na\tma\t甲\t\tunknown\t\tfixture\ta\tMIT\nb\tma\t乙\t\tunknown\t\tfixture\tb\tMIT\nc\tna\t丙\t\tunknown\t\tfixture\tc\tMIT\n'
sequences = 'a c\nb c\nc a\n'
sha = lambda text: hashlib.sha256(text.encode('utf-8')).hexdigest()
header = f'# cloudleaf-language-v1\nlexicon_sha256\t{sha(words)}\nsource_sha256\t{sha(sequences)}\nsource_id\tcloudleaf:authored-mechanical\nlicense_id\tMIT\ntokenization\tstable-entry-id-space-per-line\nusage\tmechanical\nkind\tbigram\ntraining_split\ttrain\nprevious\tnext\tcount\n'
for name, text in [('runtime.tsv', words), ('sequences.tsv', sequences), ('metadata.tsv', header),
                   ('wrong-source.tsv', header.replace(sha(sequences), '0' * 64))]:
    (out / name).write_text(text, encoding='utf-8', newline='\n')
exe = root / '.local/build/release/examples/train_language.exe'
command = [str(exe), str(out / 'runtime.tsv'), sha(words), str(out / 'sequences.tsv')]
rows = []
for label, metadata, destination, expected in [
    ('trainer-valid', 'metadata.tsv', 'model.tsv', 0),
    ('trainer-wrong-source', 'wrong-source.tsv', 'wrong-model.tsv', 1),
    ('trainer-refuses-overwrite', 'metadata.tsv', 'model.tsv', 1)]:
    result = subprocess.run([sys.executable, '-X', 'utf8', '.local/run15.py', label,
        *command, str(out / metadata), str(out / destination)], cwd=root, capture_output=True)
    row = json.loads((root / '.local/round15-runs' / label / 'result.json').read_text(encoding='utf-8'))
    assert result.returncode == expected == row['returncode']
    rows.append({'label': label, 'expected_returncode': expected, 'result': row})
assert not (out / 'wrong-model.tsv').exists()
original = (out / 'model.tsv').read_bytes()
payload_sha = hashlib.sha256(original).hexdigest()
review = [str(root / '.local/build/release/examples/input_trace.exe'), str(out / 'runtime.tsv'),
    sha(words), str(out / 'model.tsv'), payload_sha, 'bigram']
events = b'mechanical\tma na <Esc>\n'
run = subprocess.run(review, input=events, cwd=root, capture_output=True)
(out / 'trace.stdout').write_bytes(run.stdout)
(out / 'trace.stderr').write_bytes(run.stderr)
assert run.returncode == 0 and b'ranker' not in run.stderr
assert (out / 'model.tsv').read_bytes() == original
record = {'kind': 'mechanical-only', 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip(),
    'cases': rows, 'review_command': review, 'review_returncode': run.returncode,
    'events_sha256': hashlib.sha256(events).hexdigest(), 'model_sha256': payload_sha,
    'runtime_sha256': sha(words), 'sequence_sha256': sha(sequences), 'language_quality': None,
    'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'executable_sha256': hashlib.sha256(exe.read_bytes()).hexdigest()}
(out / 'result.json').write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'trainer_cases': len(rows), 'expected_rejections': 2, 'review': run.returncode, 'model_sha256': payload_sha}))

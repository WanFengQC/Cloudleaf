"""复制独立阶段工具，增加同源Witten–Bell对照，不改历史证据。"""
from pathlib import Path
import hashlib
import json
import shutil

origin = Path('C:/Users/EDY/.codex/tmp/r16-phase-profile')
out = Path('C:/Users/EDY/.codex/tmp/r16-phase-final-f2b5c5e')
root = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out.mkdir(exist_ok=False)
(out / 'src').mkdir()
for name in ('compile.py', 'measure.py', 'compress.py', 'Cargo.toml', 'cases.tsv', 'rust-toolchain.toml', 'src/main.rs'):
    shutil.copyfile(origin / name, out / name)
probe = (out / 'src/main.rs').read_text(encoding='utf-8')
assert probe.count('"character" => Ranker::CharacterBigram,') == 1
probe = probe.replace('"character" => Ranker::CharacterBigram,', '"character" | "witten" => Ranker::CharacterBigram,')
(out / 'src/main.rs').write_text(probe, encoding='utf-8', newline='\n')
measure = (out / 'measure.py').read_text(encoding='utf-8')
needle = "character = root / '.local/round16-language/character.tsv'"
assert measure.count(needle) == 1
witten = root / '.local/round16-language/character-witten.tsv'
witten_sha = hashlib.sha256(witten.read_bytes()).hexdigest()
measure = measure.replace(needle, needle + "\nwitten = root / '.local/round16-language/character-witten.tsv'\nassert sha(witten) == '" + witten_sha + "'")
measure = measure.replace("for ranker in ('structural', 'lexical', 'character'):", "for ranker in ('structural', 'lexical', 'character', 'witten'):")
measure = measure.replace("model = character if ranker == 'character' else lexical", "model = witten if ranker == 'witten' else character if ranker == 'character' else lexical")
measure = measure.replace("'character': {'path': str(character), 'sha256': sha(character)}", "'character': {'path': str(character), 'sha256': sha(character)}, 'witten': {'path': str(witten), 'sha256': sha(witten)}")
measure = measure.replace("len(record['cases']) == 144", "len(record['cases']) == 192")
(out / 'measure.py').write_text(measure, encoding='utf-8', newline='\n')
metadata = [dict(line.split('\t', 1) for line in path.read_text(encoding='utf-8').splitlines()[1:9]) for path in (root / '.local/round16-language/character.tsv', witten)]
assert metadata[0]['source_sha256'] == metadata[1]['source_sha256']
assert metadata[0]['lexicon_sha256'] == metadata[1]['lexicon_sha256']
assert metadata[0]['training_split'] == metadata[1]['training_split'] == 'train'
record = {'source_model_metadata': metadata, 'witten_model_sha256': witten_sha, 'probe_sha256': hashlib.sha256((out / 'src/main.rs').read_bytes()).hexdigest(), 'measure_tool_sha256': hashlib.sha256((out / 'measure.py').read_bytes()).hexdigest(), 'source_tool_directory': str(origin), 'production_edits': False}
(out / 'preparation.json').write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps(record, ensure_ascii=False, indent=2))

"""复用既有真实阶段探针，只在独立临时目录扩展字符观测。"""
from pathlib import Path
import hashlib
import json

root = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out = Path('C:/Users/EDY/.codex/tmp/r16-phase-profile')
out.mkdir(exist_ok=False)
(out / 'src').mkdir()
origin = root / 'docs/reports/input-round15-evidence/phase-comparison/src/main.rs'
source = origin.read_text(encoding='utf-8')
old = '"lexical" => Ranker::Lexical,'
assert source.count(old) == 1
source = source.replace(old, old + '\n            "character" => Ranker::CharacterBigram,')
assert source.count('    config.validate()?;') == 1
source = source.replace('    config.validate()?;\n', '')
old = r'\"context_units\":{}}}}}'
new = r'\"context_units\":{},\"structural_units\":{},\"character_units\":{},\"character_count\":{},\"character_unknown\":{}}}}}'
assert source.count(old) == 1
source = source.replace(old, new)
old = 'f.lexical_missing, f.bigram_units, f.context_units)'
assert source.count(old) == 1
source = source.replace(old, 'f.lexical_missing, f.bigram_units, f.context_units, f.structural_units, f.character_units, f.character_count, f.character_unknown)')
source = source.replace('cloudleaf-round15-stage-profile-v1', 'cloudleaf-round16-stage-profile-v1')
(out / 'src/main.rs').write_text(source, encoding='utf-8', newline='\n')
(out / 'Cargo.toml').write_text('''[package]
name = "r16-phase-profile"
version = "0.1.0"
edition = "2024"
rust-version = "1.90.0"

[dependencies]
cloudleaf-search = { path = "C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf/crates/cloudleaf-search" }

[workspace]

[profile.release]
lto = "thin"
codegen-units = 1
''', encoding='utf-8', newline='\n')
(out / 'rust-toolchain.toml').write_bytes((root / 'rust-toolchain.toml').read_bytes())
(out / 'cases.tsv').write_bytes((root / 'docs/reports/input-round15-evidence/phase-comparison/measurements/cases.tsv').read_bytes())
(out / 'preparation.json').write_text(json.dumps({'origin_sha256': hashlib.sha256(origin.read_bytes()).hexdigest(), 'probe_sha256': hashlib.sha256((out / 'src/main.rs').read_bytes()).hexdigest(), 'case_sha256': hashlib.sha256((out / 'cases.tsv').read_bytes()).hexdigest(), 'changes': ['explicit CharacterBigram dispatch', 'all 13 features in JSON', 'char PositionDag rejected through real query instead of early process exit', 'new tool release profile thin LTO/codegen units 1'], 'production_edits': False}, indent=2) + '\n', encoding='utf-8')
print(out)

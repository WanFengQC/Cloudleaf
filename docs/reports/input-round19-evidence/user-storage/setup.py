from pathlib import Path
import shutil
root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=Path(__file__).resolve().parent/'source'
out.mkdir(exist_ok=True)
for name in ['cloudleaf-pinyin','cloudleaf-types']:
    target=out/'crates'/name;target.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(root/'crates'/name/'Cargo.toml',target/'Cargo.toml')
    shutil.copytree(root/'crates'/name/'src',target/'src',dirs_exist_ok=True)
(out/'Cargo.toml').write_text('''[workspace]
members = ["crates/cloudleaf-user", "crates/cloudleaf-pinyin", "crates/cloudleaf-types"]
resolver = "3"
[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.90.0"
license = "MIT"
publish = false
[workspace.dependencies]
cloudleaf-types = { path = "crates/cloudleaf-types", version = "=0.1.0" }
cloudleaf-pinyin = { path = "crates/cloudleaf-pinyin", version = "=0.1.0" }
[workspace.lints.rust]
unsafe_code = "forbid"
[workspace.lints.clippy]
all = "warn"
''',encoding='utf-8')

from pathlib import Path
import re
p=Path('apps/windows-ime/Cargo.toml');s=p.read_text(encoding='utf-8');s=s.replace('"Win32_UI_Controls",','"Win32_UI_Controls", "Win32_UI_Shell",');p.write_text(s,encoding='utf-8')
p=Path('adapters/windows/isolated_input.rs');s=p.read_text(encoding='utf-8');s=re.sub(r'ui: RefCell::new\(None\),(?!\s*users:)', 'ui: RefCell::new(None),\n            users: RefCell::new(None),', s);p.write_text(s,encoding='utf-8')

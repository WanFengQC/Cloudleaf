"""追加真实字符缓存测试，测试工件不影响实际工作区。"""
from pathlib import Path
p=Path(__file__).resolve().parent
c=p/'source/crates/cloudleaf-search/src/character.rs'
c.write_text(c.read_text(encoding='utf-8')+'\n'+(p/'character-rebound-test.rs').read_text(encoding='utf-8'),encoding='utf-8')

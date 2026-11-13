"""仅交换临时源码以保存同一回归的旧实现失败证据。"""
from pathlib import Path
import sys
p=Path(__file__).resolve().parent;target=p/'source/crates/cloudleaf-search/src/search.rs'
if sys.argv[1]=='old':
    (p/'new-search-before-red.rs').write_bytes(target.read_bytes());target.write_bytes((p/'baseline-search.rs').read_bytes())
else:target.write_bytes((p/'new-search-before-red.rs').read_bytes())

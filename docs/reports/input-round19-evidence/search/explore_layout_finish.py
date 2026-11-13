"""隔离旧算法/新共享索引的实测变体，不修改生产或正式证据。"""
from pathlib import Path
import shutil
p=Path(__file__).resolve().parent;out=p/'layout-exploration';out.mkdir(exist_ok=False)
for name in ['Cargo.toml','Cargo.lock']:
    shutil.copyfile(p/'source'/name,out/name)
for folder in ['crates','probe']:
    shutil.copytree(p/'source'/folder,out/folder)
(out/'crates/cloudleaf-search/src/search.rs').write_bytes((p/'baseline-search.rs').read_bytes())
# 模型重绑留临时公开入口用于已存在字符方法，未改变查询概率。

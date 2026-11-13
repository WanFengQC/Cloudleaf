"""同步主任务已验证的共享层/模型接口，保留上一轮专项源码和日志。"""
from pathlib import Path
import hashlib,json,shutil
p=Path(__file__).resolve().parent;root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
dst=p/'user-layer-source';dst.mkdir(exist_ok=False)
for name in ['Cargo.toml','Cargo.lock']:
    shutil.copyfile(p/'source'/name,dst/name)
for folder in ['crates','probe']:
    shutil.copytree(p/'source'/folder,dst/folder)
src=root/'crates/cloudleaf-search/src'
hashes={}
for path in src.glob('*.rs'):
    shutil.copyfile(path,dst/'crates/cloudleaf-search/src'/path.name)
    hashes[path.name]=hashlib.sha256(path.read_bytes()).hexdigest()
(p/'user-layer-source-identity.json').write_text(json.dumps({'actual_repository':str(root),'production_source_files':hashes,'scope':'Exact copy of actual search crate at user-benchmark preparation; does not modify repository'},indent=2)+'\n',encoding='utf-8')

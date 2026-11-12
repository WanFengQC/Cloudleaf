"""只补提交前的少量校验附件，不再次展开或复制已有证据。"""
from pathlib import Path
import hashlib,json,shutil
root=Path(__file__).resolve().parents[1];out=root/'docs/reports/input-round18-evidence'
index=json.loads((out/'index.json').read_text(encoding='utf-8'))
items=[(root/'.local/verify_delivery18.py','closure/verify_delivery18.py'),(root/'.local/round18-precommit-verification.json','closure/precommit-verification.json'),(Path(__file__),'closure/close_evidence18.py')]
for src,name in items:
    dst=out/name;dst.parent.mkdir(parents=True,exist_ok=True);assert not dst.exists();shutil.copyfile(src,dst)
    index['files'][name]={'bytes':dst.stat().st_size,'sha256':hashlib.sha256(dst.read_bytes()).hexdigest(),'source':str(src)}
error={'scope':'Post-archive delivery-tool failure excerpt, not a product-test failure; full earlier product regression logs retained','command':['python','.local/verify_delivery18.py'],'returncode':1,'location':'verify_delivery18.py line25: assert result.returncode==0','nested_command':['python','scripts/check_workspace.py'],'nested_returncode':1,'diagnostic':'[WinError 2]; child toolchain executable absent from this shell PATH; stderr displayed with replacement characters under default console decoding','fix':'Reuse verified full-regression environment: prepend existing .cargo/bin, set PYTHONUTF8=1 and PYTHONIOENCODING=utf-8. Did not relax workspace audit.','rerun':json.loads((root/'.local/round18-precommit-verification.json').read_text(encoding='utf-8'))}
name='closure/tool-failure-and-green.json';dst=out/name;dst.write_text(json.dumps(error,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
index['files'][name]={'bytes':dst.stat().st_size,'sha256':hashlib.sha256(dst.read_bytes()).hexdigest(),'source':'Observed terminal failure and rerun result, excerpt explicitly scoped'}
index['file_count_without_index']=len(index['files']);index['total_bytes_without_index']=sum(v['bytes']for v in index['files'].values())
index['closure']='Four new delivery-check attachments; preexisting archive files unchanged; not a replay of old archive.'
(out/'index.json').write_text(json.dumps(index,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'files':len(index['files']),'bytes':index['total_bytes_without_index']}))

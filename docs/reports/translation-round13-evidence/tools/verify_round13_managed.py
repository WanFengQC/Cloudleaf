"""只在本工具新建的沙盒内，用真实固定模型验证管理程序。"""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import time

root = Path(__file__).resolve().parents[1]
out = root / '.local/round13-management'
out.mkdir(exist_ok=False)
exe = root / '.local/round13-build/release/examples/offline_models.exe'
source = root / '.local/round13-model-bundle/catalog.json'
managed = out / 'managed'
rows = []

def run(label, *args, expected=0):
    command = [str(exe), *map(str, args)]
    start = time.perf_counter()
    result = subprocess.run(command, capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
    (out / (label + '.stdout')).write_bytes(result.stdout)
    (out / (label + '.stderr')).write_bytes(result.stderr)
    rows.append({'label':label,'command':command,'returncode':result.returncode,
                 'expected_returncode':expected,'seconds':time.perf_counter()-start})
    (out / 'commands.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    assert result.returncode == expected, (label,result.returncode,result.stderr)
    return result

def digest(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file,'sha256').hexdigest()

identity = {'head':subprocess.check_output(['git','rev-parse','HEAD']).decode().strip(),
            'executable_sha256':digest(exe),'tool_sha256':digest(Path(__file__)),
            'source_catalog_sha256':digest(source)}
(out/'identity.json').write_text(json.dumps(identity,indent=2)+'\n',encoding='utf-8')
run('source-check','check',source)
first = json.loads(run('import','import',source,managed).stdout)['catalog']
catalog = Path(first)
run('duplicate-import','import',source,managed)
extra = catalog.parent/'user-note.txt'
extra.write_text('仅沙盒自有附加文件\n',encoding='utf-8')
run('uninstall-extra-file-refused','uninstall',catalog,managed,expected=1)
assert extra.is_file() and catalog.is_file()
extra.unlink()

catalog_value = json.loads(catalog.read_text(encoding='utf-8'))
manifest_path = catalog.parent/catalog_value['models'][0]['manifest']
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
weights = manifest_path.parent/next(f['path'] for f in manifest['files'] if f['role']=='weights')
with weights.open('r+b') as file:
    original=file.read(1);file.seek(0);file.write(bytes([original[0]^1]))
run('damaged-weights-not-ready','check',catalog,expected=2)
run('damaged-uninstall-refused','uninstall',catalog,managed,expected=1)
with weights.open('r+b') as file:
    file.seek(0);file.write(original)
run('restored-weights-ready','check',catalog)

tokenizer = manifest_path.parent/next(f['path'] for f in manifest['files'] if f['role']=='target_tokenizer')
missing = tokenizer.with_suffix(tokenizer.suffix+'.test-missing')
tokenizer.rename(missing)
run('missing-tokenizer-not-installed','check',catalog,expected=2)
missing.rename(tokenizer)
run('restored-tokenizer','check',catalog)

original_manifest=manifest_path.read_bytes();original_catalog=catalog.read_bytes()
manifest['files'][0]['sha256']='0'*64
manifest_path.write_text(json.dumps(manifest),encoding='utf-8')
catalog_value['models'][0]['sha256']=digest(manifest_path)
catalog.write_text(json.dumps(catalog_value),encoding='utf-8')
run('mismatched-pinned-file','check',catalog,expected=2)
manifest_path.write_bytes(original_manifest);catalog.write_bytes(original_catalog)
run('restored-manifest','check',catalog)

# 元数据版本只变更署名，不制造新权重或新许可。
version_source=out/'version-source'
shutil.copytree(source.parent,version_source)
new_catalog=version_source/'catalog.json'
new_value=json.loads(new_catalog.read_text(encoding='utf-8'))
new_manifest=version_source/new_value['models'][0]['manifest']
new_model=json.loads(new_manifest.read_text(encoding='utf-8'))
notice=next(f for f in new_model['files'] if f['role']=='notice')
notice_path=new_manifest.parent/notice['path']
with notice_path.open('ab') as file:file.write(b'\nLocal sandbox version identity test; no model change.\n')
notice['bytes']=notice_path.stat().st_size;notice['sha256']=digest(notice_path)
new_manifest.write_text(json.dumps(new_model,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
new_value['models'][0]['sha256']=digest(new_manifest)
new_catalog.write_text(json.dumps(new_value,indent=2)+'\n',encoding='utf-8')
new_path=Path(json.loads(run('metadata-version-import','import',new_catalog,managed).stdout)['catalog'])
assert new_path!=catalog and catalog.is_file()
run('metadata-version-ready','check',new_path)
run('rollback-old-ready','check',catalog)
run('uninstall-new-version','uninstall',new_path,managed)
assert not new_path.parent.exists() and catalog.is_file() and new_catalog.is_file()

# 真正终止一个导入子进程，验证暂存材料不会被当成已安装目录。
interrupted=out/'interrupted'
interrupted.mkdir()
command=[str(exe),'import',str(source),str(interrupted)]
with (out/'interrupted.stdout').open('wb') as stdout,(out/'interrupted.stderr').open('wb') as stderr:
    process=subprocess.Popen(command,stdout=stdout,stderr=stderr,creationflags=subprocess.CREATE_NO_WINDOW)
    deadline=time.monotonic()+15
    staging=[]
    while time.monotonic()<deadline and process.poll() is None:
        staging=list(interrupted.glob('.catalog-*'))
        if staging:
            process.terminate();break
        time.sleep(.001)
    code=process.wait(timeout=30)
assert staging and code!=0 and not list(interrupted.glob('catalog-*'))
rows.append({'label':'actual-interrupted-import','command':command,'returncode':code,
             'harness_termination':True,'staging_names':[p.name for p in staging],
             'physical_power_loss':False})
run('after-interruption-new-process','import',source,interrupted)
resumed=next(interrupted.glob('catalog-*/catalog.json'))
run('after-interruption-ready','check',resumed)
run('uninstall-recovered-version','uninstall',resumed,interrupted)
run('uninstall-old-version','uninstall',catalog,managed)
assert not catalog.parent.exists() and source.is_file() and weights.exists() is False
run('source-after-uninstall','check',source)
(out/'summary.json').write_text(json.dumps({'identity':identity,'checks':len(rows),'commands':rows,
    'result':'passed','physical_power_loss':'not-run','staging_preserved':True,
    'arbitrary_new_model_upgrade':'not-implemented'},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'checks':len(rows),'result':'passed'},ensure_ascii=False))

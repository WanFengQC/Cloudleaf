"""复用工程精确政策，核许可原件及固定RustSec工具，不替代源码/语言质量验收。"""
from datetime import datetime, timezone
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from check_workspace import check_packages, load_policy, repository_file

ROOT=Path(__file__).resolve().parents[1]

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def verify_licenses(root, metadata):
    packages=[p for p in metadata['packages'] if p['id'] not in metadata['workspace_members']]
    manifest=json.loads((root/'licenses/translation/manifest.json').read_bytes())
    assert {(r['package'],r['version']) for r in manifest}=={(p['name'],p['version']) for p in packages}, '许可包集合不符'
    seen=set()
    verified=[]
    for package in packages:
        directory=Path(package['manifest_path']).parent
        original={p.name:p for p in directory.iterdir() if p.is_file() and p.name.lower().startswith(('license','copying','notice','copyright','unlicense'))}
        rows=[r for r in manifest if (r['package'],r['version'])==(package['name'],package['version'])]
        matched=set()
        for row in rows:
            assert row['file'] not in seen, '重复许可副本'
            seen.add(row['file'])
            copied=repository_file(root,row['file'])
            assert row['license']==package['license'] and copied.stat().st_size==row['bytes'] and sha(copied)==row['sha256'], '许可身份不符'
            name=row['source_name']
            assert copied.name==f"{package['name']}-{package['version']}-{name}", '许可名不符'
            if row['origin']=='crate':
                assert name in original and original[name].read_bytes()==copied.read_bytes(), '许可与Cargo包原件不符'
                matched.add(name)
            else:
                assert row['origin']=='upstream-pinned-vcs' and not original, '非归档许可来源不符'
                vcs=json.loads((directory/'.cargo_vcs_info.json').read_bytes())['git']['sha1']
                repository=package['repository']
                assert repository.startswith('https://github.com/') and row['vcs_commit']==vcs, '许可VCS身份不符'
                assert row['url']==f"https://raw.githubusercontent.com/{repository.removeprefix('https://github.com/')}/{vcs}/{name}", '许可URL不符'
        assert matched==set(original), '没有保留全部包内许可'
        verified.append({key:package[key] for key in ['name','version','license','repository','rust_version']} | {'files':len(rows)})
    return verified

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--audit-binary',default='cargo-audit')
    parser.add_argument('--db',type=Path,default=Path.home()/'.cargo/advisory-db')
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    binary=shutil.which(args.audit_binary)
    if not binary:
        raise ValueError('需要仓库已采用的cargo-audit0.22.2开发工具')
    assert subprocess.check_output([binary,'--version'],encoding='utf-8').strip()=='cargo-audit 0.22.2', '审计工具版本不符'
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--offline','--format-version','1'],cwd=ROOT))
    check_packages(metadata,load_policy(ROOT),'1.90.0')
    licenses=verify_licenses(ROOT,metadata)
    lock_sha=sha(ROOT/'Cargo.lock')
    command=[binary,'audit','--file',str(ROOT/'Cargo.lock'),'--db',str(args.db),'--json','-D','warnings']
    audited=subprocess.run(command,cwd=ROOT,capture_output=True)
    rustsec=json.loads(audited.stdout)
    assert sha(ROOT/'Cargo.lock')==lock_sha, '审计时锁文件变化'
    result={'at_utc':datetime.now(timezone.utc).isoformat(),'lock_sha256':lock_sha,'audit_command':command,
        'tool_sha256':sha(Path(binary)),'returncode':audited.returncode,'licenses':licenses,'rustsec':rustsec,
        'stderr':audited.stderr.decode('utf-8'),'scope':'许可文件和已知漏洞技术检查，不证明全部供应链/平台/语言质量'}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
    assert not audited.returncode and not rustsec['vulnerabilities']['found'] and not rustsec['warnings'], '实际RustSec未通过，原文已保存'
    assert rustsec['database']['last-commit'] and rustsec['database']['last-updated'], '缺数据库身份'
    print(json.dumps({'verified':True,'packages':len(licenses),'lock_sha256':lock_sha,'database':rustsec['database']}))

if __name__=='__main__':
    main()

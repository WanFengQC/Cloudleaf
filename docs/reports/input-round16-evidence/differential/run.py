from pathlib import Path
import datetime
import hashlib
import json
import os
import subprocess
import sys
import time

root=Path(__file__).resolve().parent
output=root / "runs" / sys.argv[1]
output.mkdir(parents=True,exist_ok=False)
repo=Path(r"C:\Users\EDY\.codex\worktrees\input-round14\Cloudleaf")
source=[*sorted((repo/"crates/cloudleaf-search/src").glob("*.rs")),repo/"crates/cloudleaf-search/Cargo.toml",repo/"crates/cloudleaf-pinyin/src/lib.rs"]
def hashes():
    return {str(p.relative_to(repo)):{"sha256":hashlib.sha256(p.read_bytes()).hexdigest(),"bytes":p.stat().st_size} for p in source}
env=dict(os.environ,CARGO_TARGET_DIR=str(root/"target"))
before=hashes()
records=[]
for name,args in [("lock",["generate-lockfile","--offline"]),("run",["run","--release","--locked","--offline","--bin","r16-character-diff"])]:
    command=[r"C:\Users\EDY\.cargo\bin\cargo.exe",*args]
    start=time.monotonic()
    with (output/f"{name}.stdout.txt").open("wb") as out,(output/f"{name}.stderr.txt").open("wb") as err:
        proc=subprocess.run(command,cwd=root,env=env,stdout=out,stderr=err)
    records.append({"command":command,"returncode":proc.returncode,"elapsed_seconds":time.monotonic()-start})
    if proc.returncode:break
after=hashes()
report={"created_utc":datetime.datetime.now(datetime.timezone.utc).isoformat(),"git_head":subprocess.check_output(["git","-C",str(repo),"rev-parse","HEAD"],text=True).strip(),"commands":records,"source_before":before,"source_after":after,"source_unchanged":before==after,"files":{}}
report["summary_lines"]=[x for x in (output/"run.stdout.txt").read_text(encoding="utf-8").splitlines() if x.startswith(("SUMMARY\t","FAIL\t","EOS\t"))]
for name in ["Cargo.toml","Cargo.lock","src/main.rs","run.py","lock.stdout.txt","lock.stderr.txt","run.stdout.txt","run.stderr.txt","target/release/r16-character-diff.exe"]:
    path=output/name if name.endswith((".stdout.txt",".stderr.txt")) else root/name
    if path.exists():report["files"][name]={"bytes":path.stat().st_size,"sha256":hashlib.sha256(path.read_bytes()).hexdigest()}
(output/"summary.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
print(json.dumps({k:report[k] for k in ["commands","source_unchanged","summary_lines"]},ensure_ascii=False,indent=2))

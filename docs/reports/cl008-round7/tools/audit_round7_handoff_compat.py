"""只对照两个实际离线工具的摘要定义，不生成或批准任何任务。"""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
C = Path.home()/'.codex/worktrees/cl009-data/Cloudleaf'
sys.path.insert(0,str(ROOT/'scripts'))
import check_ime_task_handoff as b
path = C/'tools/data_pipeline/annotation_workflow.py'
spec = importlib.util.spec_from_file_location('c_annotation_workflow_compat',path)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
packet = c.pending_packet()
source = {'format_check_only':True}
assert b.FIELDS == c.FIELDS
assert all(b.annotation_digest(task) == c.annotation_digest(task) for task in packet['tasks'])
assert b.review_binding_digest(packet) == c.review_binding_digest(packet)
assert hashlib.sha256(b.canonical(source)).hexdigest() == c.source_digest(source)
value = {'B_implementation':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip(),
         'C_observed_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=C).decode().strip(),
         'C_tool_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),
         'C_tool_git_changes':subprocess.check_output(['git','diff','--name-only','--',str(path)],cwd=C).decode().splitlines(),
         'B_tool_sha256':hashlib.sha256(Path(b.__file__).read_bytes()).hexdigest(),
         'same_task_fields':True,'pending_tasks_digest_matches':len(packet['tasks']),
         'same_three_protocol_binding_digest':True,'same_source_record_digest':True,
         'human_gold_created_or_opened':False,'formal_handoff_ready':False,
         'limits':['只用C原12个空白pending任务核摘要算法，未填答案/身份/授权，不证明正式语义或人工独立。']}
(ROOT/'.local/round7-handoff-compat.json').write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(value,ensure_ascii=False))

"""研发边界输入不包含gold，保留独立原始结果。"""
from pathlib import Path
import json
root=Path(__file__).resolve().parents[1]
rows=[
    {'direction':'en-zh','text':''},
    {'direction':'zh-en','text':'   '},
    {'direction':'en-zh','text':'a'*16385},
    {'direction':'en-zh','text':'hello '*300},
    {'direction':'zh-en','text':'你好。','cancel':True},
    {'direction':'en-zh','text':'Hello.','timeout_seconds':0},
    {'direction':'en-zh','text':'Hello.'},
    {'direction':'zh-en','text':'我爱学习中文。'},
]
(root/'.local/round13-boundaries.jsonl').write_text(''.join(json.dumps(row,ensure_ascii=False)+'\n' for row in rows),encoding='utf-8')

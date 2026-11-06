from pathlib import Path
import ast
import textwrap

root=Path(__file__).resolve().parents[1]
source=Path('C:/Users/EDY/.codex/tmp/r16-corpus/prepare.py').read_text(encoding='utf-8')
tree=ast.parse(source)
lines=source.splitlines(keepends=True)
imports=[];helpers=[];omit=set()
for node in tree.body:
    if isinstance(node,(ast.Import,ast.ImportFrom)):
        imports.append(''.join(lines[node.lineno-1:node.end_lineno]))
        omit.update(range(node.lineno-1,node.end_lineno))
    elif isinstance(node,ast.FunctionDef) and node.name in ['canonical','is_han','fragments']:
        helpers.append(''.join(lines[node.lineno-1:node.end_lineno]))
        omit.update(range(node.lineno-1,node.end_lineno))
body=''.join(line for i,line in enumerate(lines) if i not in omit).lstrip()
body=body.replace("default=Path(__file__).resolve().parent", "required=True").replace("parser.add_argument('--output-dir', type=Path)","parser.add_argument('--output-dir', type=Path, required=True)")
body=body.replace("OUT.mkdir(exist_ok=False)\n",'')
needle="        raise ValueError('source identity mismatch: ' + name)\n"
body=body.replace(needle,needle+"OUT.mkdir(parents=True, exist_ok=False)\n")
result='"""固定Tatoeba上下文研发语料准备；不下载、不批准生产或生成gold。"""\n'+''.join(imports)+'\n'+'\n\n'.join(helpers)+'\n\ndef main():\n'+textwrap.indent(body,'    ')+"\n\nif __name__ == '__main__':\n    main()\n"
(root/'scripts/prepare_context_corpus.py').write_text(result,encoding='utf-8',newline='\n')
# 原始工具与处理协议保留在证据中，完整原文和owner记录留在Git外。
full=(root/'.local/full15.py').read_text(encoding='utf-8').replace('round15','round16').replace('run15.py','run16.py')
(root/'.local/full16.py').write_text(full,encoding='utf-8',newline='\n')

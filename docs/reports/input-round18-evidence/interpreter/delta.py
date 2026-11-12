from pathlib import Path
import difflib,hashlib,json
root=Path(__file__).resolve().parent
name='crates/cloudleaf-pinyin/src/scheme.rs'
old=(root/'delivered-scheme.rs').read_text(encoding='utf-8')
tests=(root/'rule-order-test-scheme.rs').read_text(encoding='utf-8')
new=(root/'scheme.rs').read_text(encoding='utf-8')
def patch(a,b):return f'diff --git a/{name} b/{name}\n'+''.join(difflib.unified_diff(a.splitlines(True),b.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
for file,a,b in [('rule-order-tests-only.patch',old,tests),('rule-order-implementation.patch',tests,new)]:
    raw=patch(a,b);(root/'delivery'/file).write_text(raw,encoding='utf-8',newline='\n')
    print(file,hashlib.sha256(raw.encode()).hexdigest())

from pathlib import Path
import difflib,hashlib,json
here=Path(__file__).resolve().parent
repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
name='crates/cloudleaf-pinyin/src/scheme.rs'
old=(here/'pre-viability-scheme.rs').read_text(encoding='utf-8')
test=(here/'viability-test-scheme.rs').read_text(encoding='utf-8')
new=(here/'scheme.rs').read_text(encoding='utf-8')
def patch(name,a,b):return f'diff --git a/{name} b/{name}\n'+''.join(difflib.unified_diff(a.splitlines(True),b.splitlines(True),fromfile='a/'+name,tofile='b/'+name))
red=patch(name,old,test)
green=patch(name,test,new)
lib=(repo/'crates/cloudleaf-pinyin/src/lib.rs').read_text(encoding='utf-8')
newlib=lib.replace('InterpretationBudget,','InterpretationBudget, InterpretationProfile,').replace('    interpret,','    interpret, interpret_profiled,')
green+=patch('crates/cloudleaf-pinyin/src/lib.rs',lib,newlib)
for file,value in [('viability-tests-only.patch',red),('viability-and-profile-implementation.patch',green)]:
    (here/'delivery'/file).write_text(value,encoding='utf-8',newline='\n')
    print(file,len(value.encode()),hashlib.sha256(value.encode()).hexdigest())

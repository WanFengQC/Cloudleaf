"""恢复已选稀疏后缀实现并同步实际共享词典契约，仅写临时工作区。"""
from pathlib import Path
import shutil
p=Path(__file__).resolve().parent
root=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
s=p/'source/crates/cloudleaf-search/src'
(s/'search.rs').write_text((p/'new-search-before-red.rs').read_text(encoding='utf-8'),encoding='utf-8')
for name in ['layer.rs','lexicon.rs','lib.rs']:
    shutil.copyfile(root/'crates/cloudleaf-search/src'/name,s/name)
# 真实模型调用尚由主任务实现；此临时公开接入口仅用于编译并调用真实缓存重绑。
m=s/'model.rs'
text=m.read_text(encoding='utf-8')
needle='impl LanguageModelSnapshot {\n'
method='''    pub fn rebound_for_user_test(&self, lexicon: &Lexicon) -> Self {
        let mut model=self.clone();
        if let Some(character)=&self.model.character {
            model.model.character=Some(character.rebound(lexicon));
        }
        model
    }
'''
if 'pub fn rebound_for_user_test'not in text:
    m.write_text(text.replace(needle,needle+method,1),encoding='utf-8')

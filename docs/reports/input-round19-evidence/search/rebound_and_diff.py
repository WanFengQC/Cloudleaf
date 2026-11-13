"""临时副本增加实际字符缓存重绑，准备原R18完整对象差分。"""
from pathlib import Path
import shutil
p=Path(__file__).resolve().parent
target=p/'source/crates/cloudleaf-search/src/character.rs'
s=target.read_text(encoding='utf-8')
start=s.index('        for entry in lexicon.entries() {')
end=s.index('        model\n    }',start)
body=s[start:end]
s=s[:start]+'        model.compile_entries(lexicon);\n'+s[end:]
where=s.index('    pub(crate) fn transition_row(')
methods='''    /// 新词层仅重编译词条字符缓存；原观测、概率和未知字符映射保持不变。
    pub(crate) fn rebound(&self, lexicon: &Lexicon) -> Self {
        let mut model=self.clone();
        model.lexicon_identity=lexicon.identity.clone();
        model.entries=Vec::with_capacity(lexicon.entries().len());
        model.compile_entries(lexicon);
        model
    }

    fn compile_entries(&mut self,lexicon:&Lexicon) {
'''+body.replace('model.','self.')+'''    }

'''
s=s[:where]+methods+s[where:]
target.write_text(s,encoding='utf-8')
shutil.copyfile(Path('C:/Users/EDY/.codex/tmp/r18-scheme-diff/src/main.rs'),p/'source/probe/src/bin/diff.rs')

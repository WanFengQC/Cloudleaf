"""替换临时试验，不叠加未采用的预处理。"""
from pathlib import Path
p=Path(__file__).resolve().parent
target=p/'source/crates/cloudleaf-search/src/search.rs'
s=(p/'baseline-search.rs').read_text(encoding='utf-8')
s=s.replace('''    pub interpretation_cost: InterpretationCost,''','''    pub interpretation_cost: InterpretationCost,
    pub user_preference_units: u32,''',1)
s=s.replace('''        .then_with(|| b.score.cmp(&a.score))''','''        .then_with(|| b.features.user_preference_units.cmp(&a.features.user_preference_units))
        .then_with(|| b.score.cmp(&a.score))''',1)
s=s.replace('''        let mut score = path.score;''','''        features.user_preference_units += self.lexicon.user_preference(edge.entry);
        let mut score = path.score;''',1)
s=s.replace('''                .then_with(|| worst.score.cmp(&score))''','''                .then_with(|| worst.features.user_preference_units.cmp(&features.user_preference_units))
                .then_with(|| worst.score.cmp(&score))''',1)
s=s.replace('''/// 在同一个只读词条图上比较搜索方法。''',(p/'sparse-exact.rs').read_text(encoding='utf-8')+'/// 在同一个只读词条图上比较搜索方法。',1)
s=s.replace('''    } else if graph.is_interpreted() {''','''    } else if config.method == Method::ExactDag && config.ranker == Ranker::CharacterBigram
        && (graph.is_interpreted() || lexicon.has_user_preferences()) {
        interpreted_character_exact(&scorer,config,&mut statistics,k,observer)
    } else if graph.is_interpreted() || lexicon.has_user_preferences() {''',1)
target.write_text(s,encoding='utf-8')
# 临时编译桩；最终补丁不包含此文件，真实API由主任务实现。
l=p/'source/crates/cloudleaf-search/src/lexicon.rs'
base=l.read_text(encoding='utf-8')
if 'pub fn user_preference('not in base:
    base=base.replace('''impl Lexicon {''','''impl Lexicon {
    pub fn user_preference(&self,_index:usize)->u32 {0}
    pub fn has_user_preferences(&self)->bool {false}''',1)
    l.write_text(base,encoding='utf-8')
c=p/'source/crates/cloudleaf-search/src/character.rs'
base=c.read_text(encoding='utf-8')
(p/'baseline-character.rs').write_text(base,encoding='utf-8')
base=base.replace('''struct Row {\n    key: u64,''','''pub(crate) struct Row {
    pub(crate) key: u64,''',1).replace('''    units: i64,\n}''','''    pub(crate) units: i64,
}''',1)
where=base.index('''    fn known(&self''')
methods='''    pub(crate) fn transition_row(&self, previous:u32)->&[Row] {
        let index=if previous==BOS {self.vocabulary.len()}else{previous as usize};
        &self.rows[self.starts[index]..self.starts[index+1]]
    }
    pub(crate) fn unobserved_bias(&self,previous:u32)->i64 {
        let index=if previous==BOS {self.vocabulary.len()}else{previous as usize};
        -self.denominators[index]
    }
    pub(crate) fn unobserved_next(&self,next:u32)->i64 {
        if self.unigram.is_empty(){0}else{self.unigram[next as usize]}
    }

'''
base=base[:where]+methods+base[where:]
c.write_text(base,encoding='utf-8')

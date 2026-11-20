from pathlib import Path

def change(name, old, new):
    p=Path(name)
    s=p.read_text(encoding='utf-8')
    assert old in s, name
    p.write_text(s.replace(old,new,1),encoding='utf-8',newline='\n')

change('crates/cloudleaf-search/src/lib.rs','mod search;','mod search;\nmod transition_cache;')
change('crates/cloudleaf-search/src/lib.rs','pub use cancellation::Cancellation;','pub use cancellation::Cancellation;\npub use transition_cache::{TransitionCache, TransitionCacheStatistics};')
change('crates/cloudleaf-search/src/schemes.rs',
'''    ) -> Result<Self, SearchError> {
        if scheme.is_default() {''',
'''    ) -> Result<Self, SearchError> {
        Self::build_with_scheme_controlled(lexicon, raw, config, scheme, None)
    }

    pub fn build_with_scheme_cached(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        cache: &mut crate::TransitionCache,
    ) -> Result<Self, SearchError> {
        Self::build_with_scheme_controlled(lexicon, raw, config, scheme, Some(cache))
    }

    fn build_with_scheme_controlled(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        cache: Option<&mut crate::TransitionCache>,
    ) -> Result<Self, SearchError> {
        if scheme.is_default() {''')
change('crates/cloudleaf-search/src/schemes.rs',
'        Self::from_phonetic(lexicon, raw, config, scheme, hypotheses)',
'        Self::from_phonetic_controlled(lexicon, raw, config, scheme, hypotheses, cache)')
change('crates/cloudleaf-search/src/schemes.rs',
'''        hypotheses: PhoneticHypotheses,
    ) -> Result<Self, SearchError> {
        if hypotheses.keys.len()''',
'''        hypotheses: PhoneticHypotheses,
    ) -> Result<Self, SearchError> {
        Self::from_phonetic_controlled(lexicon, raw, config, scheme, hypotheses, None)
    }

    fn from_phonetic_controlled(
        lexicon: &'a Lexicon,
        raw: &str,
        config: &Config,
        scheme: &SchemeConfig,
        hypotheses: PhoneticHypotheses,
        mut cache: Option<&mut crate::TransitionCache>,
    ) -> Result<Self, SearchError> {
        if hypotheses.keys.len()''')
change('crates/cloudleaf-search/src/schemes.rs',
'''        let max_edits = scheme.correction.as_ref().map_or(0, |c| c.max_edit_cost);''',
'''        if let Some(cache) = cache.as_mut() { cache.bind(lexicon); }
        let max_edits = scheme.correction.as_ref().map_or(0, |c| c.max_edit_cost);''')
change('crates/cloudleaf-search/src/schemes.rs',
'''                    let mut node = state.node;
                    let mut found = true;
                    for key in arc.syllable.bytes() {
                        if let Some(&next) = lexicon.trie[node].next.get(&key) {
                            node = next;
                        } else {
                            found = false;
                            break;
                        }
                    }
                    if !found {
                        continue;
                    }''',
'''                    let node = if let Some(cache) = cache.as_mut() {
                        let Some(node) = cache.follow(lexicon, state.node, &arc.syllable) else { continue; };
                        node
                    } else {
                        let mut node = state.node;
                        let mut found = true;
                        for key in arc.syllable.bytes() {
                            if let Some(&next) = lexicon.trie[node].next.get(&key) { node = next; }
                            else { found = false; break; }
                        }
                        if !found { continue; }
                        node
                    };''')
change('crates/cloudleaf-input/src/sentence.rs', 'use std::sync::Arc;', 'use std::{cell::RefCell, sync::Arc};')
change('crates/cloudleaf-input/src/sentence.rs',
'''    scheme: cloudleaf_pinyin::SchemeConfig,
}''',
'''    scheme: cloudleaf_pinyin::SchemeConfig,
    transitions: RefCell<cloudleaf_search::TransitionCache>,
}''')
change('crates/cloudleaf-input/src/sentence.rs',
'            scheme: cloudleaf_pinyin::SchemeConfig::default(),',
'            scheme: cloudleaf_pinyin::SchemeConfig::default(),\n            transitions: RefCell::default(),')
change('crates/cloudleaf-input/src/sentence.rs',
'        self.snapshot = snapshot;',
'        self.transitions.get_mut().clear();\n        self.snapshot = snapshot;')
change('crates/cloudleaf-input/src/sentence.rs',
'''        self.scheme = scheme;
    }

    pub(crate) fn capacity_bounds''',
'''        self.transitions.get_mut().clear();
        self.scheme = scheme;
    }

    pub(crate) fn clear_cache(&mut self) { self.transitions.get_mut().clear(); }

    pub(crate) fn cache_statistics(&self) -> cloudleaf_search::TransitionCacheStatistics {
        self.transitions.borrow().statistics()
    }

    pub(crate) fn set_cache_enabled(&mut self, enabled: bool) {
        self.transitions.get_mut().set_enabled(enabled);
    }

    pub(crate) fn capacity_bounds''')
change('crates/cloudleaf-input/src/sentence.rs',
'''        let graph = match Lattice::build_with_scheme(
            &self.snapshot.lexicon,
            input,
            &self.config,
            &self.scheme,
        )''',
'''        let graph = match Lattice::build_with_scheme_cached(
            &self.snapshot.lexicon,
            input,
            &self.config,
            &self.scheme,
            &mut self.transitions.borrow_mut(),
        )''')
change('crates/cloudleaf-input/src/lib.rs',
'''    pub fn search_capacity_bounds(&self) -> Option<SearchCapacityBounds> {''',
'''    pub fn search_cache_statistics(&self) -> Option<cloudleaf_search::TransitionCacheStatistics> {
        self.sentence.as_ref().map(sentence::SentenceBackend::cache_statistics)
    }

    pub fn search_cache_capacity_bytes(&self) -> usize {
        self.sentence.as_ref().map_or(0, |_| cloudleaf_search::TransitionCache::capacity_bytes())
    }

    pub fn set_search_cache_enabled(&mut self, enabled: bool) {
        if let Some(backend) = &mut self.sentence { backend.set_cache_enabled(enabled); }
    }

    pub fn search_capacity_bounds(&self) -> Option<SearchCapacityBounds> {''')
change('crates/cloudleaf-input/src/lib.rs',
'''    fn clear_context(&mut self, state: InputState) {
        self.preedit.clear();''',
'''    fn clear_context(&mut self, state: InputState) {
        if let Some(backend) = &mut self.sentence { backend.clear_cache(); }
        self.preedit.clear();''')

from pathlib import Path

root = Path(__file__).resolve().parent
text = (root / 'seed-search.rs').read_text(encoding='utf-8')
at = text.index('    fn completed(path: &Path, rest: &Self) -> Self {')
text = text[:at] + '''    fn compare_completed(&self, other: &Path) -> Ordering {
        self.cost
            .cmp(&other.features.interpretation_cost)
            .then_with(|| other.features.user_preference_units.cmp(&self.preference))
            .then_with(|| other.score.cmp(&self.score))
            .then_with(|| self.ids.cmp(&other.ids))
            .then_with(|| self.edges.cmp(&other.edges))
    }

''' + text[at:]
text = text.replace(
    'suffix_order(&bound, &InterpretedSuffix::prefix(complete.last().unwrap()))',
    'bound.compare_completed(complete.last().unwrap())',
)
text = text.replace('''suffix_order(
                &current.bound,
                &InterpretedSuffix::prefix(complete.last().unwrap()),
            )''', 'current.bound.compare_completed(complete.last().unwrap())')
text = text.replace('''        for tail in &rest.edges {
            if scorer.cancelled() || !expand_allowed(stats, config) { return Vec::new(); }
            candidate = scorer.extend(&candidate, *tail, None, observer).unwrap();
        }''', '''        // 后缀标签已按剩余编辑数验证，沿该链扩展必然可行。
        for tail in &rest.edges {
            if scorer.cancelled() || !expand_allowed(stats, config) { return Vec::new(); }
            candidate = scorer.extend(&candidate, *tail, None, observer).unwrap();
        }''')
text = text.replace('''    let mut pending = BinaryHeap::from([InterpretedPending {
        bound: best.clone(),''', '''    if !resident_allowed(stats, labels + complete.len() + 2, config) {
        return Vec::new();
    }
    let mut pending = BinaryHeap::from([InterpretedPending {
        bound: best.clone(),''')
(root / 'source/crates/cloudleaf-search/src/search.rs').write_text(text, encoding='utf-8', newline='\n')

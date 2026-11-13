#[cfg(test)]
mod rebound_tests {
    use super::BOS;
    use crate::{LanguageModelSnapshot,Lexicon};
    use std::sync::Arc;

    #[test]
    fn user_entries_recompile_character_cache_without_rebinding_old_indices() {
        let header="# cloudleaf-lexicon-v2\nid\tpinyin\ttext\tfrequency\tfrequency_kind\tannotation\tsource\tsource_record_id\tlicense_id\n";
        let base=format!("{header}a\tni\t你\t\tunknown\t\tmechanical\ta\tMIT\nb\thao\t好\t\tunknown\t\tmechanical\tb\tMIT\n");
        let combined=format!("{base}user:1\tni hao\t你好\t\tunknown\t\tuser\tuser:1\tuser-private\n");
        let lexicon=Lexicon::from_tsv(&base).unwrap();
        let target=Lexicon::from_tsv(&combined).unwrap();
        let sha="7".repeat(64);
        for kind in ["character_bigram","character_bigram_witten_bell"] {
            let metadata=format!("# cloudleaf-language-v1\nlexicon_sha256\t{sha}\nsource_sha256\t{sha}\nsource_id\tcloudleaf:mechanical-rebound\nlicense_id\tMIT\ntokenization\tunicode-scalar-v1\nusage\tmechanical\nkind\t{kind}\ntraining_split\ttrain\nprevious\tnext\tcount\n");
            let model=LanguageModelSnapshot::train(&metadata,&lexicon,&sha,&[vec!["你".into(),"好".into()],vec!["你".into(),"你".into()]]).unwrap();
            let original=model.model().character.as_ref().unwrap();
            let rebound=original.rebound(&target);
            assert_eq!(original.entries.len(),2);
            assert_eq!(rebound.entries.len(),3);
            assert!(Arc::ptr_eq(&rebound.lexicon_identity,&target.identity));
            assert!(!Arc::ptr_eq(&rebound.lexicon_identity,&original.lexicon_identity));
            for previous in std::iter::once(BOS).chain(0..original.vocabulary.len()as u32) {
                for next in 0..original.vocabulary.len()as u32 {
                    assert_eq!(original.transition(previous,next),rebound.transition(previous,next));
                }
            }
            let a=original.entry(0);let b=original.entry(1);let added=rebound.entry(2);
            assert_eq!((added.first,added.last,added.characters),(a.first,b.last,2));
            assert_eq!(added.internal,original.transition(a.last,b.first));
            assert_eq!(added.unknown,a.unknown+b.unknown);
            assert_eq!(rebound.entry(0).internal,original.entry(0).internal);
        }
    }
}

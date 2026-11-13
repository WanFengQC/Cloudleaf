"""在原R18完整对象探针上增加实际用户层和偏好，保留原语料与比较断言。"""
from pathlib import Path
p=Path(__file__).resolve().parent
src=p/'source/probe/src/bin/diff.rs'
s=src.read_text(encoding='utf-8')
s=s.replace('    let lex = Lexicon::from_tsv(&data).unwrap();','''    let base_lex = Lexicon::from_tsv(&data).unwrap();
    let header="# cloudleaf-lexicon-v2\\nid\\tpinyin\\ttext\\tfrequency\\tfrequency_kind\\tannotation\\tsource\\tsource_record_id\\tlicense_id\\n";
    let users=format!("{header}user:1\\tni hao\\t你好\\t\\tunknown\\t\\tuser\\tuser:1\\tLicenseRef-User-Private\\nuser:2\\tni hao\\t拟好\\t\\tunknown\\t\\tuser\\tuser:2\\tLicenseRef-User-Private\\nuser:3\\tzhong guo ren\\t中国人\\t\\tunknown\\t\\tuser\\tuser:3\\tLicenseRef-User-Private\\nuser:4\\tna\\t那\\t\\tunknown\\t\\tuser\\tuser:4\\tLicenseRef-User-Private\\n");
    let preferences=base_lex.entries().iter().enumerate().map(|(i,e)|(e.id.clone(),[0,1,3,8][i%4])).chain([("user:1".into(),1),("user:2".into(),8),("user:3".into(),4),("user:4".into(),2)]).collect();
    let lex=base_lex.with_user_layer(&users,&preferences).unwrap();
    println!("USER_LAYER\\t{}\\t{:?}",lex.entries().len(),preferences);
''',1)
s=s.replace('    let variants = [','    let variants = [("full",SchemeConfig::default()),',1)
s=s.replace('schemes=4','schemes=5')
(src.parent/'preference_diff.rs').write_text(s,encoding='utf-8')

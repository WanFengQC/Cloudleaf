"""两个版本使用完全相同的独立搜索调用探针。"""
from pathlib import Path
p=Path(__file__).resolve().parent
for name in ['source','baseline-source-finish']:
    path=p/name/'probe/src/main.rs'
    s=path.read_text(encoding='utf-8')
    s=s.replace('ranker: Ranker::CharacterBigram,','ranker: if model.metadata().kind=="lexical"{Ranker::Lexical}else{Ranker::CharacterBigram},')
    s=s.replace('method: Method::ExactDag,','method: if model.metadata().kind=="lexical"{Method::PositionDag}else{Method::ExactDag},')
    path.write_text(s,encoding='utf-8')

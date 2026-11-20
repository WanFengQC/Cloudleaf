from pathlib import Path
import shutil
p=Path(__file__).resolve().parent
s=p/'source/crates/cloudleaf-search/src/search.rs'
shutil.copy(p/'source/target/release/r19-search-probe.exe',p/'baseline.exe')
shutil.copy(s,p/'baseline-search.rs')
t=s.read_text(encoding='utf-8')
start=t.index('fn interpreted_character_exact')
a,b=t[:start],t[start:]
b=b.replace('    let model = scorer.model.unwrap().character.as_ref().unwrap();','    let measuring = std::time::Instant::now();\n    let model = scorer.model.unwrap().character.as_ref().unwrap();',1)
b=b.replace('    let mut pending = BinaryHeap::from([InterpretedPending {','    eprintln!("PROFILE suffix_ns={} labels={} edges={} raw={}", measuring.elapsed().as_nanos(), labels, scorer.graph.edges.len(), scorer.graph.raw);\n    let path_measuring = std::time::Instant::now();\n    let mut materialized = 0usize;\n    let mut pending = BinaryHeap::from([InterpretedPending {',1)
b=b.replace('            let Some(next) = scorer.extend(&current.path, *edge_index, None, observer) else {','            materialized += 1;\n            let Some(next) = scorer.extend(&current.path, *edge_index, None, observer) else {',1)
b=b.replace('                return Vec::new();\n            }\n            materialized','                eprintln!("PROFILE FAIL path_ns={} labels={} seen={} pending={} materialized={} expansions={} raw={}",path_measuring.elapsed().as_nanos(),labels,seen_count,pending.len(),materialized,stats.expansions,scorer.graph.raw);\n                return Vec::new();\n            }\n            materialized',1)
b=b.replace('    complete\n}', '    eprintln!("PROFILE DONE path_ns={} labels={} seen={} materialized={} expansions={} raw={}",path_measuring.elapsed().as_nanos(),labels,seen_count,materialized,stats.expansions,scorer.graph.raw);\n    complete\n}',1)
s.write_text(a+b,encoding='utf-8',newline='\n')

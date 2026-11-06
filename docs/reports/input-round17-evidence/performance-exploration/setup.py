from pathlib import Path
import difflib
import hashlib
import json
import shutil
import subprocess

repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
root=Path(__file__).resolve().parent
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo).decode().strip()=='521472db18ecd7926f6e5b58b0afd7b5da674a07'
before={str(p.relative_to(repo)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (repo/'crates/cloudleaf-search/src').glob('*.rs')}
base=(repo/'crates/cloudleaf-search/src/search.rs').read_text(encoding='utf-8')

old='''        let mut next = path.clone();
        next.score = score;
        next.features = features;
        next.last_character = last_character;
        next.text.push_str(&entry.text);
        next.ids.push(edge.entry);
        next.edges.push(edge_index);
        Some(next)'''
new='''        let mut text = String::with_capacity(path.text.len() + entry.text.len());
        text.push_str(&path.text);
        text.push_str(&entry.text);
        let mut ids = Vec::with_capacity(path.ids.len() + 1);
        ids.extend_from_slice(&path.ids);
        ids.push(edge.entry);
        let mut edges = Vec::with_capacity(path.edges.len() + 1);
        edges.extend_from_slice(&path.edges);
        edges.push(edge_index);
        Some(Path { text, score, ids, edges, features, last_character })'''
assert base.count(old)==1
once=base.replace(old,new)
forward_old='''            let next = scorer
                .extend(&current.path, *edge_index, None, observer)
                .unwrap();
            let bound = next.score + rest.score;
            if complete.len() == k && bound < complete.last().unwrap().score {
                continue;
            }
            // 同位置相同正文有相同字符分和未来状态；不同词典分词只保留最优稳定ID。
            if seen[edge.end]
                .get(&next.text)
                .is_some_and(|ids| ids <= &next.ids)
            {
                continue;
            }'''
forward_new='''            let bound = current.path.score + model.transition(
                current.path.last_character.unwrap_or(crate::character::BOS), entry.first,
            ) + entry.internal + rest.score;
            if complete.len() == k && bound < complete.last().unwrap().score {
                continue;
            }
            // 先复用正文暂存并借用ID比较，淘汰分支不再复制整条路径。
            scratch.clear();
            scratch.push_str(&current.path.text);
            scratch.push_str(&scorer.lexicon.entries[edge.entry].text);
            if seen[edge.end].get(scratch.as_str()).is_some_and(|ids| {
                current.path.ids.iter().copied().chain(std::iter::once(edge.entry))
                    .cmp(ids.iter().copied()) != Ordering::Less
            }) {
                continue;
            }
            let next = scorer.extend(&current.path, *edge_index, None, observer).unwrap();'''
assert once.count(forward_old)==1
filtered=once.replace(forward_old,forward_new).replace('    let mut seen_count = 1;\n','    let mut seen_count = 1;\n    let mut scratch = String::new();\n')

variants={'baseline':base,'one-allocation':once,'prefilter':filtered}
for label,search in variants.items():
    directory=root/label
    directory.mkdir(exist_ok=False)
    for name in ['cloudleaf-types','cloudleaf-pinyin','cloudleaf-search']:
        shutil.copytree(repo/'crates'/name,directory/'crates'/name)
    cargo=(repo/'Cargo.toml').read_text(encoding='utf-8')
    start=cargo.index('members = [');end=cargo.index(']',start)+1
    cargo=cargo[:start]+'members = ["crates/cloudleaf-types", "crates/cloudleaf-pinyin", "crates/cloudleaf-search", "probe"]'+cargo[end:]
    (directory/'Cargo.toml').write_text(cargo,encoding='utf-8',newline='\n')
    shutil.copyfile(repo/'rust-toolchain.toml',directory/'rust-toolchain.toml')
    (directory/'crates/cloudleaf-search/src/search.rs').write_text(search,encoding='utf-8',newline='\n')
    probe=directory/'probe';(probe/'src').mkdir(parents=True)
    (probe/'Cargo.toml').write_text('[package]\nname="r17-perf-probe"\nversion="0.1.0"\nedition="2024"\n[dependencies]\ncloudleaf-search={path="../crates/cloudleaf-search"}\n',encoding='utf-8')
    shutil.copyfile(Path('C:/Users/EDY/.codex/tmp/r16-phase-final-f2/src/main.rs'),probe/'src/main.rs')
    (root/(label+'.patch')).write_text(''.join(difflib.unified_diff(base.splitlines(keepends=True),search.splitlines(keepends=True),fromfile='a/crates/cloudleaf-search/src/search.rs',tofile='b/crates/cloudleaf-search/src/search.rs')),encoding='utf-8',newline='\n')
(root/'source-identity.json').write_text(json.dumps({'base':'521472db18ecd7926f6e5b58b0afd7b5da674a07','original_source':before,'variants':{name:hashlib.sha256(value.encode()).hexdigest() for name,value in variants.items()}},indent=2)+'\n',encoding='utf-8')
print('created baseline and two minimal temporary variants')

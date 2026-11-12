from pathlib import Path
import difflib,hashlib,json
root=Path(__file__).resolve().parent;repo=Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
out=root/'tie-fix';out.mkdir(exist_ok=False)
p=repo/'crates/cloudleaf-search/src/search.rs';before=p.read_text(encoding='utf-8');after=before
after=after.replace('''    last_character: Option<u32>,
}''','''    last_character: Option<u32>,
    interpreted: bool,
}''')
after=after.replace('''        .then_with(|| a.ids.cmp(&b.ids))
}''','''        .then_with(|| a.ids.cmp(&b.ids))
        .then_with(|| if a.interpreted || b.interpreted { a.edges.cmp(&b.edges) } else { Ordering::Equal })
}''')
needle='''                        .cmp(worst.ids.iter().copied())
                })
                == Ordering::Greater'''
replacement='''                        .cmp(worst.ids.iter().copied())
                })
                .then_with(|| {
                    if self.graph.is_interpreted() {
                        path.edges.iter().copied().chain(std::iter::once(edge_index)).cmp(worst.edges.iter().copied())
                    } else { Ordering::Equal }
                })
                == Ordering::Greater'''
assert needle in after
after=after.replace(needle,replacement)
needle='''            features,
            last_character,
        })'''
replacement='''            features,
            last_character,
            interpreted: self.graph.is_interpreted(),
        })'''
assert needle in after
after=after.replace(needle,replacement)
assert after!=before
def patch(a,b,path):return ''.join(difflib.unified_diff(a.splitlines(keepends=True),b.splitlines(keepends=True),fromfile='a/'+path,tofile='b/'+path))
test=(root/'tie-test.rs').read_text(encoding='utf-8')
new='diff --git a/crates/cloudleaf-search/tests/scheme_provenance.rs b/crates/cloudleaf-search/tests/scheme_provenance.rs\nnew file mode 100644\n'+''.join(difflib.unified_diff([],test.splitlines(keepends=True),fromfile='/dev/null',tofile='b/crates/cloudleaf-search/tests/scheme_provenance.rs'))
(out/'tests-only.patch').write_text(new,encoding='utf-8',newline='\n')
(out/'implementation.patch').write_text(patch(before,after,'crates/cloudleaf-search/src/search.rs'),encoding='utf-8',newline='\n')
(out/'search.rs').write_text(after,encoding='utf-8',newline='\n')
(out/'manifest.json').write_text(json.dumps({'base_search_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'patches':{n:hashlib.sha256((out/n).read_bytes()).hexdigest()for n in ['tests-only.patch','implementation.patch']}},indent=2)+'\n',encoding='utf-8',newline='\n')

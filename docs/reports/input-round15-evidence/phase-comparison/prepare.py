from pathlib import Path

out = Path(__file__).resolve().parent
root = Path('C:/Users/EDY/.codex/worktrees/input-round14/Cloudleaf')
source = (root / 'crates/cloudleaf-search/examples/resource_phase_probe.rs').read_text(encoding='utf-8')
source = source.replace('Config, Lattice, Lexicon, MatchKind, Method, Phase, Profile, Ranker, SearchError, SearchResult,',
                        'Config, LanguageModelSnapshot, Lattice, Lexicon, Method, Phase, Profile, Ranker, SearchError, SearchResult,')
start = source.index('fn details(')
end = source.index('\nfn equal(', start)
details = r'''fn details(value: &Result<(Lattice<'_>, SearchResult), SearchError>) -> String {
    match value {
        Err(error) => format!("{{\"status\":\"rejected\",\"error_kind\":{},\"error_message\":{},\"candidates\":[]}}", quote(&format!("{:?}", error.kind)), quote(error.message)),
        Ok((_, result)) => {
            let candidates = result.candidates.iter().map(|candidate| {
                let f = &candidate.features;
                format!("{{\"text\":{},\"score\":{},\"entry_ids\":[{}],\"edge_indices\":{:?},\"features\":{{\"words\":{},\"prefixes\":{},\"frequency_units\":{},\"missing_frequencies\":{},\"lexical_units\":{},\"lexical_observed\":{},\"lexical_missing\":{},\"bigram_units\":{},\"context_units\":{}}}}}",
                    quote(&candidate.text), candidate.score, strings(&candidate.entry_ids), candidate.edge_indices,
                    f.words, f.prefixes, f.frequency_units, f.missing_frequencies, f.lexical_units, f.lexical_observed, f.lexical_missing, f.bigram_units, f.context_units)
            }).collect::<Vec<_>>().join(",");
            let s = &result.statistics;
            format!("{{\"status\":\"ok\",\"candidates\":[{candidates}],\"raw_fallback\":{},\"waiting\":{},\"guaranteed\":{},\"finite_pool_only\":{},\"statistics\":{{\"graph_edges\":{},\"expansions\":{},\"resident_high_water\":{},\"beam_pruned\":{},\"budget\":{}}}}}",
                quote(&result.raw_fallback), result.waiting, result.guaranteed, result.finite_pool_only,
                s.graph_edges, s.expansions, s.resident_high_water, s.beam_pruned,
                s.budget.map(|value| quote(&format!("{value:?}"))).unwrap_or("null".into()))
        }
    }
}
'''
source = source[:start] + details + source[end:]
source = source.replace('if args.len() != 11 {', 'if args.len() != 14 {')
source = source.replace('参数：runtime cases method beam iterations warmup max_edges max_expansions max_resident output_k',
                        '参数：runtime cases method ranker model lexicon_sha beam iterations warmup max_edges max_expansions max_resident output_k')
source = source.replace('let iterations: usize = args[5].parse()?;', 'let iterations: usize = args[8].parse()?;')
source = source.replace('let warmup: usize = args[6].parse()?;', 'let warmup: usize = args[9].parse()?;')
source = source.replace('ranker: Ranker::Structural,', '''ranker: match args[4].as_str() {
            "structural" => Ranker::Structural,
            "lexical" => Ranker::Lexical,
            _ => return Err("未知评分".into()),
        },''')
source = source.replace('beam_width: args[4].parse()?', 'beam_width: args[7].parse()?')
source = source.replace('max_edges: args[7].parse()?', 'max_edges: args[10].parse()?')
source = source.replace('max_expansions: args[8].parse()?', 'max_expansions: args[11].parse()?')
source = source.replace('max_resident_paths: args[9].parse()?', 'max_resident_paths: args[12].parse()?')
source = source.replace('output_k: args[10].parse()?', 'output_k: args[13].parse()?')
source = source.replace('let load_ns = started.elapsed().as_nanos();', '''let load_ns = started.elapsed().as_nanos();
    let model_text = fs::read_to_string(&args[5])?;
    let snapshot = LanguageModelSnapshot::from_tsv(&model_text, &lexicon, &args[6])?;
    drop(model_text);
    let model = Some(snapshot.model());''')
source = source.replace('search_graph(&lexicon, &graph, &config, None, None)', 'search_graph(&lexicon, &graph, &config, model, None)')
source = source.replace('profile_query(&lexicon, fields[2], &config, None, None)', 'profile_query(&lexicon, fields[2], &config, model, None)')
source = source.replace('details(&left,fields[2],&lexicon,config.output_k)', 'details(&left)')
source = source.replace('details(&right,fields[2],&lexicon,config.output_k)', 'details(&right)')
source = source.replace('details(last.as_ref().unwrap(), fields[2], &lexicon, config.output_k)', 'details(last.as_ref().unwrap())')
source = source.replace('"result_debug\\\":{}}}', '"result_debug\\\":{},\\\"graph\\\":{}}}') if False else source
source = source.replace('quote(&full_result)));', 'quote(&full_result)));')
source = source.replace('"cloudleaf-stage-profile-v2', '"cloudleaf-round15-stage-profile-v1')
(out / 'src').mkdir(parents=True, exist_ok=True)
(out / 'src/main.rs').write_text(source, encoding='utf-8', newline='\n')
print('Prepared actual shared search/profiling wrapper; no measurement executed.')

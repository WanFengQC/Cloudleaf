//! 临时分配计数探针；不进入产品代码、不把累计请求字节冒称峰值内存。
use cloudleaf_search::{Config,LanguageModelSnapshot,Lattice,Lexicon,Method,Ranker,search_graph};
use std::alloc::{GlobalAlloc,Layout,System};
use std::sync::atomic::{AtomicU64,Ordering};
use std::{fs,hint::black_box};

struct Counting;
static ALLOC:AtomicU64=AtomicU64::new(0);
static REALLOC:AtomicU64=AtomicU64::new(0);
static BYTES:AtomicU64=AtomicU64::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self,layout:Layout)->*mut u8 {ALLOC.fetch_add(1,Ordering::Relaxed);BYTES.fetch_add(layout.size() as u64,Ordering::Relaxed);unsafe {System.alloc(layout)}}
    unsafe fn dealloc(&self,ptr:*mut u8,layout:Layout){unsafe {System.dealloc(ptr,layout)}}
    unsafe fn realloc(&self,ptr:*mut u8,layout:Layout,new_size:usize)->*mut u8 {REALLOC.fetch_add(1,Ordering::Relaxed);BYTES.fetch_add(new_size as u64,Ordering::Relaxed);unsafe {System.realloc(ptr,layout,new_size)}}
}
#[global_allocator] static ALLOCATOR:Counting=Counting;

fn main(){
    let args:Vec<_>=std::env::args().collect();
    let lex=Lexicon::from_tsv(&fs::read_to_string(&args[1]).unwrap()).unwrap();
    let model=LanguageModelSnapshot::from_tsv(&fs::read_to_string(&args[2]).unwrap(),&lex,&args[3]).unwrap();
    let cases=fs::read_to_string(&args[4]).unwrap();
    let cfg=Config{method:Method::ExactDag,ranker:Ranker::CharacterBigram,output_k:10,..Config::default()};
    println!("id\tallocations\treallocations\tcumulative_requested_bytes\texpansions\tresident_high_water\tbudget\ttop1");
    for row in cases.lines().skip(2){
        let fields:Vec<_>=row.split('\t').collect();
        let graph=Lattice::build(&lex,fields[2],&cfg).unwrap();
        ALLOC.store(0,Ordering::Relaxed);REALLOC.store(0,Ordering::Relaxed);BYTES.store(0,Ordering::Relaxed);
        let result=black_box(search_graph(&lex,&graph,&cfg,Some(model.model()),None).unwrap());
        let allocated=ALLOC.load(Ordering::Relaxed);let reallocated=REALLOC.load(Ordering::Relaxed);let bytes=BYTES.load(Ordering::Relaxed);
        println!("{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{:?}",fields[0],allocated,reallocated,bytes,result.statistics.expansions,result.statistics.resident_high_water,result.statistics.budget,result.candidates.first().map(|c|c.text.as_str()));
    }
}

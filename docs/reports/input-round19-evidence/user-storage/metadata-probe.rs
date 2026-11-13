use std::os::windows::fs::MetadataExt;
fn main() {
    let m=std::fs::metadata(std::env::current_exe().unwrap()).unwrap();
    println!("{} {:?}",m.file_attributes(),m.number_of_links());
}

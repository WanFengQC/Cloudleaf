fn main() {
    let file = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
    let _ = file.try_lock();
    let _ = file.unlock();
}

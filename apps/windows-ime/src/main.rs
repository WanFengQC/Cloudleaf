//! 显式安装、卸载和诊断入口，不修改默认输入法。

fn main() {
    #[cfg(windows)]
    if let Err(error) = cloudleaf_windows_ime::native::manage(std::env::args().skip(1).collect()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("此管理工具只支持 Windows");
        std::process::exit(1);
    }
}

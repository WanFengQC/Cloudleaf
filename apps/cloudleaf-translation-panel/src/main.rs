#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use std::path::PathBuf;
    let mut args = std::env::args_os().skip(1);
    let mut directory = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("Cloudleaf/TranslationPanel"));
    let mut dictionary = None;
    while let Some(arg) = args.next() {
        if arg == "--settings-dir" {
            directory = args.next().map(PathBuf::from);
        } else if arg == "--dictionary" {
            dictionary = args.next().map(PathBuf::from);
        } else {
            cloudleaf_translation_panel::windows_ui::show_error(
                "参数：--settings-dir 目录 --dictionary catalog.json",
            );
            return;
        }
    }
    let Some(directory) = directory else {
        cloudleaf_translation_panel::windows_ui::show_error(
            "无法确定设置目录，请使用 --settings-dir 指定",
        );
        return;
    };
    if let Err(error) = cloudleaf_translation_panel::windows_ui::run(directory, dictionary) {
        cloudleaf_translation_panel::windows_ui::show_error(&error);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("当前平台不支持 Windows 翻译面板；跨平台翻译核心仍可独立使用");
    std::process::exit(2);
}

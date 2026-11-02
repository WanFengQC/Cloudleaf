use cloudleaf_offline::{import_catalog, inspect_catalog, uninstall_catalog};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["check", catalog] => {
            let result = inspect_catalog(Path::new(catalog));
            println!("{}", serde_json::to_string_pretty(&result)?);
            if result.error.is_some() { std::process::exit(2); }
        }
        ["import", catalog, managed] => {
            let imported = import_catalog(Path::new(catalog), Path::new(managed))?;
            println!("{}", serde_json::json!({"catalog": imported}));
        }
        ["uninstall", catalog, managed] => uninstall_catalog(Path::new(catalog), Path::new(managed))?,
        _ => return Err("usage: offline_models check CATALOG | import CATALOG MANAGED_ROOT | uninstall CATALOG MANAGED_ROOT".into()),
    }
    Ok(())
}

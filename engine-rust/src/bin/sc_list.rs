//! List CASC paths in a retail StarCraft installation.
//!
//! ```text
//! sc_list --install "/path/to/StarCraft" --prefix game/consoles
//! sc_list --install "/path/to/StarCraft" --contains cmdbtns
//! ```

use std::path::PathBuf;

use galaxycraft::retail::RetailInstall;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut install_path: Option<PathBuf> = None;
    let mut prefix = String::new();
    let mut contains: Option<String> = None;
    let mut suffix = String::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--install" => {
                install_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--prefix" => {
                prefix = args[i + 1].clone();
                i += 2;
            }
            "--contains" => {
                contains = Some(args[i + 1].to_lowercase());
                i += 2;
            }
            "--suffix" => {
                suffix = args[i + 1].clone();
                i += 2;
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let install = match install_path {
        Some(path) => RetailInstall::open(&path).expect("open install"),
        None => RetailInstall::from_env().expect("no install; pass --install"),
    };
    let paths = install.list(&prefix, &suffix);
    for path in &paths {
        if let Some(needle) = &contains
            && !path.to_lowercase().contains(needle)
        {
            continue;
        }
        println!("{path}");
    }
    eprintln!("{} paths", paths.len());
}

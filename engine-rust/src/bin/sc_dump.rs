//! Dump the CASC path namespace to a local directory for offline inspection.
//!
//! The retail install is never modified.  This is the search tool used when a
//! menu asset is not exposed by `RetailInstall::materialize` (for example the
//! 3D front-end frame and the CEF host configuration).
//!
//! ```text
//! sc_dump --install "/path/to/StarCraft" --out /tmp/sc-casc \
//!         --include glue/ --include webui/ --include shaders
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use galaxycraft::retail::RetailInstall;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut install_path = None;
    let mut out = None;
    let mut includes: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--install" => {
                install_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--include" => {
                includes.push(args[i + 1].to_ascii_lowercase());
                i += 2;
            }
            other => {
                eprintln!("unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(out) = out else {
        eprintln!("--out DIR is required");
        return ExitCode::from(2);
    };
    let install = match install_path {
        Some(path) => match RetailInstall::open(&path) {
            Ok(install) => install,
            Err(e) => {
                eprintln!("cannot open {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => match RetailInstall::from_env() {
            Some(install) => install,
            None => {
                eprintln!("no install; pass --install");
                return ExitCode::FAILURE;
            }
        },
    };
    let mut paths = install.list("", "");
    paths.sort();
    let mut written = 0usize;
    let mut bytes = 0u64;
    for path in paths {
        if !includes.is_empty()
            && !includes
                .iter()
                .any(|p| path.to_ascii_lowercase().starts_with(p))
        {
            continue;
        }
        // Skip media that cannot be a menu frame/host config; keep the dump
        // small enough to search quickly.
        let lower = path.to_ascii_lowercase();
        if [".wav", ".ogg", ".webm", ".smk", ".anim", ".chk", ".srt"]
            .iter()
            .any(|e| lower.ends_with(e))
        {
            continue;
        }
        let Some(data) = install.read(&path) else {
            continue;
        };
        let dst = out.join(&path);
        if let Some(parent) = dst.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                continue;
            }
        }
        if let Ok(mut f) = std::fs::File::create(&dst) {
            if f.write_all(&data).is_ok() {
                written += 1;
                bytes += data.len() as u64;
            }
        }
    }
    println!(
        "dumped {written} files ({bytes} bytes) into {}",
        out.display()
    );
    ExitCode::SUCCESS
}

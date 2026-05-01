//! Shared CLI helpers: load config, walk files, parse → `ExtractedFiles` map.
//!
//! Subcommands (`check`, `why`) all need the same setup, so it lives once here.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lc_core::pipeline::ExtractedFiles;
use lc_core::rule::Rule;
use lc_io::config::{self, Config};

pub const CONFIG_FILE: &str = ".layer-conform.json";

pub fn load_config() -> Result<Config> {
    let path = Path::new(CONFIG_FILE);
    config::load(path).with_context(|| format!("loading {CONFIG_FILE}"))
}

pub fn compile_rules(cfg: &Config) -> Result<Vec<Rule>> {
    lc_io::compile::compile_rules(cfg).with_context(|| "compiling rules")
}

/// Walk source files under `root` (default cwd), parse each, build an
/// `ExtractedFiles` map keyed by path **relative to `root`**.
///
/// `restrict_to` filters the walk: when non-empty, only those files are parsed.
pub fn extract_workspace(root: &Path, restrict_to: &[PathBuf]) -> Result<ExtractedFiles> {
    let files = if restrict_to.is_empty() {
        lc_io::walker::walk_source_files(root)
    } else {
        restrict_to
            .iter()
            .map(|p| if p.is_absolute() { p.clone() } else { root.join(p) })
            .collect()
    };

    // Discover the Rust workspace once so each .rs file gets the same
    // resolution context (use map → relative-layer signatures). When no
    // Cargo.toml lives at `root` we fall back to name-only encoding.
    let rs_workspace = lc_rs::workspace::discover_workspace(root)
        .with_context(|| "discovering Rust workspace")?;

    let mut out: ExtractedFiles = HashMap::with_capacity(files.len());
    for path in files {
        let rel = match path.strip_prefix(root) {
            Ok(r) => r.to_path_buf(),
            Err(_) => path.clone(),
        };
        let key = rel.to_string_lossy().into_owned();
        let source = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let funcs = match path.extension().and_then(|s| s.to_str()) {
            Some("rs") => match &rs_workspace {
                Some(ws) => lc_rs::parse_file_with_context(
                    &source,
                    &lc_rs::FileContext { workspace: ws, file_path: &path },
                ),
                None => lc_rs::parse_file(&source),
            },
            _ => lc_ts::parse_file(&source),
        };
        out.insert(key, funcs);
    }
    Ok(out)
}

/// Convert a possibly-absolute `path` into a string relative to `root` so it
/// can be used as a key into `ExtractedFiles`.
pub fn relativize(root: &Path, path: &Path) -> String {
    if path.is_absolute() {
        path.strip_prefix(root)
            .map_or_else(
                |_| path.to_string_lossy().into_owned(),
                |p| p.to_string_lossy().into_owned(),
            )
    } else {
        path.to_string_lossy().into_owned()
    }
}

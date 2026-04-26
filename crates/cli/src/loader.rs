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

    let mut out: ExtractedFiles = HashMap::with_capacity(files.len());
    for path in files {
        let rel = match path.strip_prefix(root) {
            Ok(r) => r.to_path_buf(),
            Err(_) => path.clone(),
        };
        let key = rel.to_string_lossy().into_owned();
        let source = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let funcs = lc_ts::parse_file(&source);
        out.insert(key, funcs);
    }
    Ok(out)
}

//! `layer-conform check`: drive the full pipeline from `.layer-conform.json`.

use std::io::stdout;
use std::path::{Path, PathBuf};

use anyhow::Result;
use lc_core::pipeline;

use crate::loader;
use crate::reporter;

pub struct CheckOpts {
    pub paths: Vec<PathBuf>,
    pub explain: Option<PathBuf>,
    pub threshold: Option<f64>,
    pub no_color: bool,
    pub json: bool,
}

/// Returns process exit code (0 = clean, 1 = deviations found).
pub fn run(opts: CheckOpts) -> Result<i32> {
    let cfg = loader::load_config()?;
    let rules = loader::compile_rules(&cfg)?;
    let root = std::env::current_dir()?;
    let files = loader::extract_workspace(&root, &opts.paths)?;
    let mut deviations = pipeline::detect_deviations(&rules, &files, opts.threshold)?;

    if let Some(explain_path) = &opts.explain {
        let key = relativize(&root, explain_path);
        deviations.retain(|d| d.file == key);
    }

    let exit = i32::from(!deviations.is_empty());
    let mut out = stdout().lock();
    if opts.json {
        reporter::render_json(&mut out, &deviations)?;
    } else {
        reporter::render_text(
            &mut out,
            &deviations,
            reporter::TextOpts { no_color: opts.no_color },
        )?;
    }
    Ok(exit)
}

fn relativize(root: &Path, path: &Path) -> String {
    if path.is_absolute() {
        path.strip_prefix(root).map_or_else(|_| path.to_string_lossy().into_owned(), |p| p.to_string_lossy().into_owned())
    } else {
        path.to_string_lossy().into_owned()
    }
}


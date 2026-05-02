//! `layer-conform check`: drive the full pipeline from `.layer-conform.json`.
//!
//! Walks every file matched by each rule's `applyTo` glob, scores every
//! function against its golden(s), and renders the resulting deviations as
//! text or JSON. `--explain <FILE>` post-filters the output — the pipeline
//! still ran globally so cross-rule overlap remains visible without it.

use std::io::stdout;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use layer_conform_core::pipeline;

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
    let deviations = pipeline::detect_deviations(&rules, &files, opts.threshold)?;
    let focus_file = resolve_focus_file(&opts.explain, &root, &files)?;

    let mut out = stdout().lock();
    if opts.json {
        reporter::render_json(&mut out, &deviations, focus_file.as_deref())?;
    } else {
        reporter::render_text(
            &mut out,
            &deviations,
            reporter::TextOpts { no_color: opts.no_color, focus_file },
        )?;
    }
    Ok(i32::from(!deviations.is_empty()))
}

fn resolve_focus_file(
    explain: &Option<PathBuf>,
    root: &std::path::Path,
    files: &layer_conform_core::pipeline::ExtractedFiles,
) -> Result<Option<String>> {
    let Some(explain_path) = explain else { return Ok(None) };
    let key = loader::relativize(root, explain_path);
    if !files.contains_key(&key) {
        return Err(anyhow!(
            "--explain target `{key}` was not walked; check `applyTo` globs in .layer-conform.json"
        ));
    }
    Ok(Some(key))
}

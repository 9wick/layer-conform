//! `layer-conform why <FILE>`: show every rule that touches FILE plus its
//! similarity to each golden — even when there is no deviation.
//!
//! Unlike `check --explain`, `why` always lists *all* candidate functions in
//! the file and *all* goldens, so users can see the full scoring matrix.

use std::io::stdout;
use std::path::PathBuf;

use anyhow::Result;
use layer_conform_core::explain;

use crate::loader;
use crate::reporter;

pub struct WhyOpts {
    pub file: PathBuf,
    pub no_color: bool,
    pub json: bool,
}

pub fn run(opts: WhyOpts) -> Result<i32> {
    let cfg = loader::load_config()?;
    let rules = loader::compile_rules(&cfg)?;
    let root = std::env::current_dir()?;
    let files = loader::extract_workspace(&root, &[])?;
    let key = loader::relativize(&root, &opts.file);

    let report = explain::build_why_report(&rules, &files, &key)?;

    let mut out = stdout().lock();
    if opts.json {
        reporter::render_why_json(&mut out, &report)?;
    } else {
        reporter::render_why_text(
            &mut out,
            &report,
            reporter::TextOpts { no_color: opts.no_color, focus_file: None },
        )?;
    }
    Ok(0)
}

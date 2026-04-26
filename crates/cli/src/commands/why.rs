//! `layer-conform why <FILE>`: show every rule that touches FILE plus its
//! similarity to each golden — even when there is no deviation.
//!
//! Unlike `check --explain`, `why` always lists *all* candidate functions in
//! the file and *all* goldens, so users can see the full scoring matrix.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lc_core::pipeline;
use lc_core::rule::Rule;
use lc_core::similarity::{aggregate, jaccard_sorted, Weights};
use lc_core::{tsed, FunctionRef};

use crate::loader;

pub fn run(file: PathBuf) -> Result<i32> {
    let cfg = loader::load_config()?;
    let rules = loader::compile_rules(&cfg)?;
    let root = std::env::current_dir()?;
    let key = relativize(&root, &file);

    // Need ALL files extracted so we can resolve goldens that live elsewhere.
    let files = loader::extract_workspace(&root, &[])?;
    let funcs_in_file = files
        .get(&key)
        .with_context(|| format!("{key} did not extract to any function (was it walked?)"))?;

    let matching_rules: Vec<&Rule> =
        lc_core::matcher::matching_rules(Path::new(&key), &rules);

    if matching_rules.is_empty() {
        println!("{key}  →  no rule matches");
        return Ok(0);
    }

    let weights = Weights::default();
    for rule in matching_rules {
        let goldens = resolve_goldens_for_explain(rule, &files);
        for func in funcs_in_file {
            if func.ignore.is_some() {
                println!(
                    "{key}:{symbol}  →  rule `{rule_id}` (skipped: layer-conform-ignore)",
                    symbol = func.symbol,
                    rule_id = rule.id,
                );
                continue;
            }
            println!(
                "{key}:{symbol}  →  rule `{rule_id}`",
                symbol = func.symbol,
                rule_id = rule.id,
            );
            for (gsel, golden_func) in &goldens {
                let shape = tsed::tsed(&func.tree, &golden_func.tree);
                let calls = jaccard_sorted(&func.calls, &golden_func.calls);
                let imports = jaccard_sorted(&func.imports, &golden_func.imports);
                let signature =
                    if func.signature == golden_func.signature { 1.0 } else { 0.0 };
                let s = aggregate(shape, calls, imports, signature, weights);
                let threshold = rule.threshold.unwrap_or(0.7);
                let verdict = if s.overall >= threshold { "CONFORM" } else { "DEVIATION" };
                println!(
                    "  vs golden {gf}:{gs}  overall={:.3} (threshold {threshold:.2}) → {verdict}",
                    s.overall,
                    gf = gsel.file,
                    gs = gsel.symbol,
                );
                println!(
                    "    shape={:.3}  calls={:.3}  imports={:.3}  signature={:.3}",
                    s.shape, s.calls, s.imports, s.signature,
                );
            }
        }
    }
    Ok(0)
}

fn resolve_goldens_for_explain<'f>(
    rule: &Rule,
    files: &'f pipeline::ExtractedFiles,
) -> Vec<(lc_core::rule::GoldenSelector, &'f FunctionRef)> {
    rule.goldens
        .iter()
        .filter_map(|g| {
            files.get(&g.file).and_then(|funcs| {
                funcs
                    .iter()
                    .find(|f| f.symbol.as_str() == g.symbol)
                    .map(|f| (g.clone(), f))
            })
        })
        .collect()
}

fn relativize(root: &Path, path: &Path) -> String {
    if path.is_absolute() {
        path.strip_prefix(root).map_or_else(|_| path.to_string_lossy().into_owned(), |p| p.to_string_lossy().into_owned())
    } else {
        path.to_string_lossy().into_owned()
    }
}


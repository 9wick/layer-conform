//! Pure orchestrator: rules + extracted functions → deviations.
//!
//! No I/O, no parsing — the caller pre-extracts every function and supplies
//! them indexed by file path. This keeps the core testable with hand-built
//! `FunctionRef`s and lets language adapters parallelize file parsing.

use std::collections::HashMap;
use std::path::Path;

use compact_str::CompactString;

use crate::deviation::{diff_sets, pick_best, Deviation, Differences, GoldenMatch};
use crate::rule::{GoldenSelector, Rule};
use crate::similarity::{aggregate, jaccard_sorted, SimilarityScore, Weights};
use crate::tsed;
use crate::FunctionRef;

const DEFAULT_THRESHOLD: f64 = 0.7;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("rule `{rule_id}`: golden `{file}:{symbol}` not found in extracted files")]
    GoldenNotFound { rule_id: String, file: String, symbol: String },
}

/// All functions extracted from one source file, keyed by relative path string.
pub type ExtractedFiles = HashMap<String, Vec<FunctionRef>>;

/// Run the deviation pipeline.
///
/// `threshold_override` (e.g. `--threshold` from CLI) wins over each rule's
/// `threshold`, which wins over the built-in default of 0.7.
pub fn detect_deviations(
    rules: &[Rule],
    files: &ExtractedFiles,
    threshold_override: Option<f64>,
) -> Result<Vec<Deviation>, PipelineError> {
    let weights = Weights::default();
    let mut out = Vec::new();
    for rule in rules {
        if rule.disabled {
            continue;
        }
        let goldens = resolve_goldens(rule, files)?;
        let threshold = threshold_override
            .or(rule.threshold)
            .unwrap_or(DEFAULT_THRESHOLD);

        for (file_path, funcs) in files {
            if !rule.matches(Path::new(file_path)) {
                continue;
            }
            for func in funcs {
                if func.ignore.is_some() {
                    continue;
                }
                if is_golden(&goldens, file_path, &func.symbol) {
                    continue;
                }
                let matches = score_against_all(func, &goldens, weights);
                let (best, sorted) = pick_best(matches);
                if best.similarity.overall < threshold {
                    out.push(build_deviation(rule, file_path, func, best, sorted, &goldens));
                }
            }
        }
    }
    Ok(out)
}

fn resolve_goldens<'f>(
    rule: &Rule,
    files: &'f ExtractedFiles,
) -> Result<Vec<(GoldenSelector, &'f FunctionRef)>, PipelineError> {
    let mut goldens = Vec::with_capacity(rule.goldens.len());
    for g in &rule.goldens {
        let funcs = files.get(&g.file).ok_or_else(|| PipelineError::GoldenNotFound {
            rule_id: rule.id.clone(),
            file: g.file.clone(),
            symbol: g.symbol.clone(),
        })?;
        let func = funcs
            .iter()
            .find(|f| f.symbol.as_str() == g.symbol)
            .ok_or_else(|| PipelineError::GoldenNotFound {
                rule_id: rule.id.clone(),
                file: g.file.clone(),
                symbol: g.symbol.clone(),
            })?;
        goldens.push((g.clone(), func));
    }
    Ok(goldens)
}

fn is_golden(
    goldens: &[(GoldenSelector, &FunctionRef)],
    file_path: &str,
    symbol: &CompactString,
) -> bool {
    goldens
        .iter()
        .any(|(g, _)| g.file == file_path && g.symbol == symbol.as_str())
}

fn score_against_all(
    func: &FunctionRef,
    goldens: &[(GoldenSelector, &FunctionRef)],
    weights: Weights,
) -> Vec<GoldenMatch> {
    goldens
        .iter()
        .map(|(sel, golden_func)| GoldenMatch {
            golden: sel.clone(),
            similarity: score_pair(func, golden_func, weights),
        })
        .collect()
}

/// Score two functions on the 4 axes (shape / calls / imports / signature) and
/// return the aggregated `SimilarityScore`. Pub so command-level explainers
/// (`why`) can reuse the exact same scoring as `detect_deviations`.
pub fn score_pair(actual: &FunctionRef, golden: &FunctionRef, weights: Weights) -> SimilarityScore {
    let shape = tsed::tsed(&actual.tree, &golden.tree);
    let calls = jaccard_sorted(&actual.calls, &golden.calls);
    let imports = jaccard_sorted(&actual.imports, &golden.imports);
    let signature = if actual.signature == golden.signature { 1.0 } else { 0.0 };
    aggregate(shape, calls, imports, signature, weights)
}

fn build_deviation(
    rule: &Rule,
    file_path: &str,
    func: &FunctionRef,
    best: GoldenMatch,
    sorted: Vec<GoldenMatch>,
    goldens: &[(GoldenSelector, &FunctionRef)],
) -> Deviation {
    let golden_func = goldens
        .iter()
        .find(|(g, _)| *g == best.golden)
        .map(|(_, f)| *f)
        .expect("matched golden must be in resolved set");
    let (missing_calls, extra_calls) = diff_sets(&golden_func.calls, &func.calls);
    let (missing_imports, extra_imports) = diff_sets(&golden_func.imports, &func.imports);
    Deviation {
        rule_id: rule.id.clone(),
        file: file_path.to_string(),
        symbol: func.symbol.clone(),
        matched_golden: best.golden.clone(),
        all_golden_scores: sorted,
        similarity: best.similarity,
        differences: Differences {
            missing_calls,
            extra_calls,
            missing_imports,
            extra_imports,
        },
    }
}

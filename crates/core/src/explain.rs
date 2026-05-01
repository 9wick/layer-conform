//! Build a full scoring matrix for the `why` command.
//!
//! Mirrors `pipeline::detect_deviations` but emits *every* (rule, function,
//! golden) score — including matches above threshold and ignored functions —
//! so callers can render the full reasoning behind a deviation/conform call.

use std::path::Path;

use compact_str::CompactString;

use crate::matcher;
use crate::pipeline::{score_pair, ExtractedFiles};
use crate::rule::{GoldenSelector, Rule};
use crate::similarity::{SimilarityScore, Weights};
use crate::FunctionRef;

const DEFAULT_THRESHOLD: f64 = 0.7;

#[derive(Clone, Debug)]
pub struct WhyReport {
    pub file: String,
    pub entries: Vec<WhyEntry>,
}

#[derive(Clone, Debug)]
pub enum WhyEntry {
    /// Function carried a `layer-conform-ignore` directive and was skipped.
    Skipped { rule_id: String, symbol: CompactString },
    /// Function was scored against every resolvable golden of the rule.
    Scored {
        rule_id: String,
        symbol: CompactString,
        threshold: f64,
        matches: Vec<WhyMatch>,
    },
}

#[derive(Clone, Debug)]
pub struct WhyMatch {
    pub golden: GoldenSelector,
    pub similarity: SimilarityScore,
}

#[derive(Debug, thiserror::Error)]
pub enum ExplainError {
    #[error("{file} did not extract to any function (was it walked?)")]
    FileNotWalked { file: String },
}

/// Build the per-(rule, func, golden) scoring matrix for `target_file`.
/// `entries` is empty when no rule's `applyTo` glob matched the file —
/// callers should treat that as the "no rule matches" case.
pub fn build_why_report(
    rules: &[Rule],
    files: &ExtractedFiles,
    target_file: &str,
) -> Result<WhyReport, ExplainError> {
    let funcs_in_file = files
        .get(target_file)
        .ok_or_else(|| ExplainError::FileNotWalked { file: target_file.to_string() })?;

    let matching = matcher::matching_rules(Path::new(target_file), rules);
    let mut entries: Vec<WhyEntry> = Vec::new();
    if matching.is_empty() {
        return Ok(WhyReport { file: target_file.to_string(), entries });
    }

    let weights = Weights::default();
    for rule in matching {
        let goldens = resolve_goldens(rule, files);
        let threshold = rule.threshold.unwrap_or(DEFAULT_THRESHOLD);
        for func in funcs_in_file {
            if func.ignore.is_some() {
                entries.push(WhyEntry::Skipped {
                    rule_id: rule.id.clone(),
                    symbol: func.symbol.clone(),
                });
                continue;
            }
            let matches: Vec<WhyMatch> = goldens
                .iter()
                .map(|(gsel, golden_func)| WhyMatch {
                    golden: gsel.clone(),
                    similarity: score_pair(func, golden_func, weights),
                })
                .collect();
            entries.push(WhyEntry::Scored {
                rule_id: rule.id.clone(),
                symbol: func.symbol.clone(),
                threshold,
                matches,
            });
        }
    }

    Ok(WhyReport { file: target_file.to_string(), entries })
}

fn resolve_goldens<'f>(
    rule: &Rule,
    files: &'f ExtractedFiles,
) -> Vec<(GoldenSelector, &'f FunctionRef)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::TreeNode;
    use crate::{FunctionKind, Signature};
    use globset::{Glob, GlobSet, GlobSetBuilder};
    use std::collections::HashMap;

    fn glob_set(patterns: &[&str]) -> GlobSet {
        let mut b = GlobSetBuilder::new();
        for p in patterns {
            b.add(Glob::new(p).expect("valid glob"));
        }
        b.build().expect("build glob set")
    }

    fn func(symbol: &str) -> FunctionRef {
        let mut tree = TreeNode::branch(crate::tree::NodeKind::Block, vec![]);
        tree.finalize();
        let ast_hash = tree.canonical_hash();
        FunctionRef {
            symbol: symbol.into(),
            kind: FunctionKind::FunctionDeclaration,
            start_line: 0,
            end_line: 0,
            byte_range: (0, 0),
            tree,
            signature: Signature { param_count: 0 },
            calls: vec![],
            imports: vec![],
            ast_hash,
            ignore: None,
        }
    }

    #[test]
    fn missing_target_file_returns_error() {
        let rules: Vec<Rule> = vec![];
        let files: ExtractedFiles = HashMap::new();
        let err = build_why_report(&rules, &files, "missing.ts").unwrap_err();
        match err {
            ExplainError::FileNotWalked { file } => assert_eq!(file, "missing.ts"),
        }
    }

    #[test]
    fn no_rule_match_yields_empty_entries() {
        let mut files: ExtractedFiles = HashMap::new();
        files.insert("src/x.ts".into(), vec![func("foo")]);
        let rule = Rule {
            id: "repos".into(),
            goldens: vec![],
            apply_to: glob_set(&["src/repos/**/*.ts"]),
            ignore: glob_set(&[]),
            threshold: None,
            disabled: false,
        };
        let r = build_why_report(&[rule], &files, "src/x.ts").unwrap();
        assert!(r.entries.is_empty());
    }

    #[test]
    fn scored_entry_per_function_with_one_match_per_golden() {
        let mut files: ExtractedFiles = HashMap::new();
        files.insert("src/x.ts".into(), vec![func("a"), func("b")]);
        files.insert("src/g.ts".into(), vec![func("g")]);
        let rule = Rule {
            id: "r".into(),
            goldens: vec![GoldenSelector { file: "src/g.ts".into(), symbol: "g".into() }],
            apply_to: glob_set(&["src/x.ts"]),
            ignore: glob_set(&[]),
            threshold: Some(0.5),
            disabled: false,
        };
        let r = build_why_report(&[rule], &files, "src/x.ts").unwrap();
        assert_eq!(r.entries.len(), 2);
        for e in &r.entries {
            match e {
                WhyEntry::Scored { matches, threshold, .. } => {
                    assert_eq!(matches.len(), 1);
                    assert!((threshold - 0.5).abs() < f64::EPSILON);
                }
                WhyEntry::Skipped { .. } => panic!("unexpected skipped"),
            }
        }
    }
}

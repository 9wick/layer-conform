//! Translate the parsed `Config` into runtime `lc_core::Rule`s with
//! pre-compiled `GlobSet`s. Done once per CLI invocation.

use globset::{Glob, GlobSet, GlobSetBuilder};
use lc_core::rule::{GoldenSelector, Rule};

use crate::config::Config;

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("invalid glob `{pattern}`: {source}")]
    InvalidGlob {
        pattern: String,
        #[source]
        source: globset::Error,
    },
    #[error("failed to build glob set: {0}")]
    BuildGlobSet(#[source] globset::Error),
}

pub fn compile_rules(cfg: &Config) -> Result<Vec<Rule>, CompileError> {
    cfg.rules.iter().map(compile_rule).collect()
}

fn compile_rule(r: &crate::config::Rule) -> Result<Rule, CompileError> {
    let apply_to = build_globset(&r.apply_to)?;
    let ignore = build_globset(&r.ignore)?;
    let goldens = r
        .goldens
        .iter()
        .map(|g| GoldenSelector { file: g.file.clone(), symbol: g.symbol.clone() })
        .collect();
    Ok(Rule {
        id: r.id.clone(),
        goldens,
        apply_to,
        ignore,
        threshold: r.threshold,
        disabled: r.disabled,
    })
}

fn build_globset(patterns: &[String]) -> Result<GlobSet, CompileError> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        let glob = Glob::new(p).map_err(|source| CompileError::InvalidGlob {
            pattern: p.clone(),
            source,
        })?;
        b.add(glob);
    }
    b.build().map_err(CompileError::BuildGlobSet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::from_str;
    use std::path::Path;

    #[test]
    fn compiles_simple_rule() {
        let cfg = from_str(
            r#"{ "version": 1, "rules": [{
                "id": "r",
                "golden": "src/a.ts:a",
                "applyTo": "src/**/*.ts"
            }]}"#,
        )
        .unwrap();
        let rules = compile_rules(&cfg).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].matches(Path::new("src/foo.ts")));
        assert!(!rules[0].matches(Path::new("lib/foo.ts")));
    }

    #[test]
    fn compiles_ignore_globs() {
        let cfg = from_str(
            r#"{ "version": 1, "rules": [{
                "id": "r",
                "golden": "src/a.ts:a",
                "applyTo": "src/**/*.ts",
                "ignore": "src/legacy/**"
            }]}"#,
        )
        .unwrap();
        let rules = compile_rules(&cfg).unwrap();
        assert!(!rules[0].matches(Path::new("src/legacy/old.ts")));
        assert!(rules[0].matches(Path::new("src/new.ts")));
    }

    #[test]
    fn rejects_invalid_glob() {
        let cfg = from_str(
            r#"{ "version": 1, "rules": [{
                "id": "r",
                "golden": "src/a.ts:a",
                "applyTo": "src/[unclosed"
            }]}"#,
        )
        .unwrap();
        let err = compile_rules(&cfg).unwrap_err();
        assert!(matches!(err, CompileError::InvalidGlob { .. }));
    }
}

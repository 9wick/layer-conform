//! Implements `layer-conform check` for a single (file, symbol) vs golden pair.

use std::{fs, path::PathBuf};

use anyhow::{anyhow, Context};
use lc_core::{
    deviation::diff_sets,
    similarity::{aggregate, jaccard_sorted, Weights},
    tsed,
    FunctionRef,
};

use crate::reporter::Report;

pub fn run_check(file: PathBuf, symbol: &str, golden_spec: &str) -> anyhow::Result<()> {
    let (g_path, g_symbol) = parse_golden_spec(golden_spec)?;

    let actual = load_function(&file, symbol)?;
    let golden = load_function(&PathBuf::from(&g_path), &g_symbol)?;

    let shape = tsed::tsed(&actual.tree, &golden.tree);
    let calls = jaccard_sorted(&actual.calls, &golden.calls);
    let imports = jaccard_sorted(&actual.imports, &golden.imports);
    let signature = if actual.signature == golden.signature { 1.0 } else { 0.0 };
    let score = aggregate(shape, calls, imports, signature, Weights::default());

    let (missing_calls, extra_calls) = diff_sets(&golden.calls, &actual.calls);

    let report = Report {
        file: file.to_str().unwrap_or("<file>"),
        symbol,
        golden_file: &g_path,
        golden_symbol: &g_symbol,
        score,
        missing_calls: &missing_calls,
        extra_calls: &extra_calls,
    };
    print!("{}", report.render());
    Ok(())
}

fn parse_golden_spec(spec: &str) -> anyhow::Result<(String, String)> {
    let (path, name) = spec
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("--golden must be \"<path>:<symbol>\", got {spec}"))?;
    if path.is_empty() || name.is_empty() {
        return Err(anyhow!("--golden has empty part: {spec}"));
    }
    Ok((path.to_string(), name.to_string()))
}

fn load_function(path: &PathBuf, symbol: &str) -> anyhow::Result<FunctionRef> {
    let src = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let funcs = lc_ts::parse_file(&src);
    funcs
        .into_iter()
        .find(|f| f.symbol == symbol)
        .ok_or_else(|| anyhow!("symbol `{symbol}` not found in {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_golden_spec_splits_on_last_colon() {
        let (p, s) = parse_golden_spec("src/foo.ts:useFoo").unwrap();
        assert_eq!(p, "src/foo.ts");
        assert_eq!(s, "useFoo");
    }

    #[test]
    fn parse_golden_spec_rejects_missing_colon() {
        assert!(parse_golden_spec("foo").is_err());
    }

    #[test]
    fn parse_golden_spec_rejects_empty_part() {
        assert!(parse_golden_spec(":foo").is_err());
        assert!(parse_golden_spec("foo:").is_err());
    }
}

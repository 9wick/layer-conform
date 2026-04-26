//! Render `Deviation` results in text or JSON form.

use std::io::Write;

use lc_core::deviation::Deviation;
use serde::Serialize;

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_DIM: &str = "\x1b[2m";

#[derive(Debug, Clone, Copy)]
pub struct TextOpts {
    pub no_color: bool,
}

pub fn render_text(
    out: &mut impl Write,
    deviations: &[Deviation],
    opts: TextOpts,
) -> std::io::Result<()> {
    let (red, dim, reset) = if opts.no_color {
        ("", "", "")
    } else {
        (ANSI_RED, ANSI_DIM, ANSI_RESET)
    };

    if deviations.is_empty() {
        writeln!(out, "No deviations.")?;
        return Ok(());
    }

    for d in deviations {
        writeln!(
            out,
            "{red}DEVIATION{reset} {file}:{symbol} (rule `{rule}`)",
            file = d.file,
            symbol = d.symbol,
            rule = d.rule_id,
        )?;
        writeln!(
            out,
            "  vs golden {gf}:{gs}",
            gf = d.matched_golden.file,
            gs = d.matched_golden.symbol,
        )?;
        writeln!(
            out,
            "  overall={:.3}  shape={:.3} calls={:.3} imports={:.3} signature={:.3}",
            d.similarity.overall,
            d.similarity.shape,
            d.similarity.calls,
            d.similarity.imports,
            d.similarity.signature,
        )?;
        if !d.differences.missing_calls.is_empty() {
            let names: Vec<&str> =
                d.differences.missing_calls.iter().map(compact_str::CompactString::as_str).collect();
            writeln!(out, "  missing calls: [{}]", names.join(", "))?;
        }
        if !d.differences.extra_calls.is_empty() {
            let names: Vec<&str> =
                d.differences.extra_calls.iter().map(compact_str::CompactString::as_str).collect();
            writeln!(out, "  extra calls:   [{}]", names.join(", "))?;
        }
        if d.all_golden_scores.len() > 1 {
            for (idx, m) in d.all_golden_scores.iter().enumerate().skip(1) {
                writeln!(
                    out,
                    "  {dim}#{n} golden {gf}:{gs} = {:.3}{reset}",
                    m.similarity.overall,
                    n = idx + 1,
                    gf = m.golden.file,
                    gs = m.golden.symbol,
                )?;
            }
        }
    }
    writeln!(out, "{} deviation(s).", deviations.len())?;
    Ok(())
}

#[derive(Serialize)]
struct JsonReport<'a> {
    version: u32,
    deviations: Vec<JsonDeviation<'a>>,
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonDeviation<'a> {
    rule_id: &'a str,
    file: &'a str,
    symbol: &'a str,
    matched_golden: JsonGolden<'a>,
    all_golden_scores: Vec<JsonGoldenMatch<'a>>,
    similarity: JsonSimilarity,
    differences: JsonDifferences<'a>,
}

#[derive(Serialize)]
struct JsonGolden<'a> {
    file: &'a str,
    symbol: &'a str,
}

#[derive(Serialize)]
struct JsonGoldenMatch<'a> {
    golden: JsonGolden<'a>,
    similarity: JsonSimilarity,
}

#[derive(Serialize)]
struct JsonSimilarity {
    overall: f64,
    shape: f64,
    calls: f64,
    imports: f64,
    signature: f64,
}

#[derive(Serialize)]
struct JsonDifferences<'a> {
    missing_calls: Vec<&'a str>,
    extra_calls: Vec<&'a str>,
    missing_imports: Vec<&'a str>,
    extra_imports: Vec<&'a str>,
}

#[derive(Serialize)]
struct JsonSummary {
    deviations: usize,
}

pub fn render_json(out: &mut impl Write, deviations: &[Deviation]) -> std::io::Result<()> {
    let report = JsonReport {
        version: 1,
        deviations: deviations.iter().map(json_deviation).collect(),
        summary: JsonSummary { deviations: deviations.len() },
    };
    serde_json::to_writer_pretty(&mut *out, &report)
        .map_err(std::io::Error::other)?;
    writeln!(out)
}

fn json_deviation(d: &Deviation) -> JsonDeviation<'_> {
    JsonDeviation {
        rule_id: &d.rule_id,
        file: &d.file,
        symbol: d.symbol.as_str(),
        matched_golden: JsonGolden {
            file: &d.matched_golden.file,
            symbol: &d.matched_golden.symbol,
        },
        all_golden_scores: d
            .all_golden_scores
            .iter()
            .map(|m| JsonGoldenMatch {
                golden: JsonGolden { file: &m.golden.file, symbol: &m.golden.symbol },
                similarity: similarity(&m.similarity),
            })
            .collect(),
        similarity: similarity(&d.similarity),
        differences: JsonDifferences {
            missing_calls: d.differences.missing_calls.iter().map(compact_str::CompactString::as_str).collect(),
            extra_calls: d.differences.extra_calls.iter().map(compact_str::CompactString::as_str).collect(),
            missing_imports: d.differences.missing_imports.iter().map(compact_str::CompactString::as_str).collect(),
            extra_imports: d.differences.extra_imports.iter().map(compact_str::CompactString::as_str).collect(),
        },
    }
}

fn similarity(s: &lc_core::similarity::SimilarityScore) -> JsonSimilarity {
    JsonSimilarity {
        overall: s.overall,
        shape: s.shape,
        calls: s.calls,
        imports: s.imports,
        signature: s.signature,
    }
}

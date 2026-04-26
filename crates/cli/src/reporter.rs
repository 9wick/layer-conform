//! Text reporter for MVP single-pair comparison output.

use lc_core::similarity::SimilarityScore;

pub struct Report<'a> {
    pub file: &'a str,
    pub symbol: &'a str,
    pub golden_file: &'a str,
    pub golden_symbol: &'a str,
    pub score: SimilarityScore,
    pub missing_calls: &'a [compact_str::CompactString],
    pub extra_calls: &'a [compact_str::CompactString],
}

impl<'a> Report<'a> {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "{}:{} vs {}:{}\n",
            self.file, self.symbol, self.golden_file, self.golden_symbol
        ));
        out.push_str(&format!(
            "  overall={:.3}  shape={:.3}  calls={:.3}  imports={:.3}  signature={:.3}\n",
            self.score.overall,
            self.score.shape,
            self.score.calls,
            self.score.imports,
            self.score.signature,
        ));
        if !self.missing_calls.is_empty() {
            out.push_str(&format!("  missing calls: {:?}\n", self.missing_calls));
        }
        if !self.extra_calls.is_empty() {
            out.push_str(&format!("  extra calls:   {:?}\n", self.extra_calls));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compact_str::CompactString;

    #[test]
    fn renders_header_and_scores() {
        let r = Report {
            file: "a.ts",
            symbol: "useFoo",
            golden_file: "b.ts",
            golden_symbol: "useBar",
            score: SimilarityScore { overall: 0.5, shape: 0.6, calls: 0.4, imports: 1.0, signature: 0.0 },
            missing_calls: &[CompactString::from("useSWR")],
            extra_calls: &[CompactString::from("fetch")],
        };
        let s = r.render();
        assert!(s.contains("a.ts:useFoo"));
        assert!(s.contains("b.ts:useBar"));
        assert!(s.contains("overall=0.500"));
        assert!(s.contains("missing calls"));
        assert!(s.contains("extra calls"));
    }
}

//! Deviation report: what differs from the closest golden.

use compact_str::CompactString;

use crate::rule::GoldenSelector;
use crate::similarity::SimilarityScore;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Differences {
    pub missing_calls: Vec<CompactString>,
    pub extra_calls: Vec<CompactString>,
    pub missing_imports: Vec<CompactString>,
    pub extra_imports: Vec<CompactString>,
}

/// One scored comparison of an actual function against one golden.
#[derive(Clone, Debug, PartialEq)]
pub struct GoldenMatch {
    pub golden: GoldenSelector,
    pub similarity: SimilarityScore,
}

/// Result of comparing one function against all goldens of a rule.
///
/// `matched_golden` is the highest-scoring golden (= `all_golden_scores[0]`).
/// `similarity` mirrors `matched_golden.similarity` for ergonomic access.
#[derive(Clone, Debug, PartialEq)]
pub struct Deviation {
    pub rule_id: String,
    pub file: String,
    pub symbol: CompactString,
    pub matched_golden: GoldenSelector,
    pub all_golden_scores: Vec<GoldenMatch>,
    pub similarity: SimilarityScore,
    pub differences: Differences,
}

/// Pick the best golden out of a non-empty list of scored matches.
///
/// Sorts `all_golden_scores` descending by `overall`. Returns `(matched, sorted)`
/// where `matched` is a clone of the top entry.
///
/// # Panics
///
/// Panics if `scores` is empty — callers must guarantee at least one golden.
pub fn pick_best(scores: Vec<GoldenMatch>) -> (GoldenMatch, Vec<GoldenMatch>) {
    assert!(!scores.is_empty(), "pick_best requires at least one golden");
    let mut sorted = scores;
    sorted.sort_by(|a, b| {
        b.similarity
            .overall
            .partial_cmp(&a.similarity.overall)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let best = sorted[0].clone();
    (best, sorted)
}

/// Both `golden` and `actual` MUST be sorted ascending.
pub fn diff_sets(golden: &[CompactString], actual: &[CompactString]) -> (Vec<CompactString>, Vec<CompactString>) {
    // missing = golden - actual, extra = actual - golden
    let (mut i, mut j) = (0_usize, 0_usize);
    let (mut missing, mut extra) = (Vec::new(), Vec::new());
    while i < golden.len() && j < actual.len() {
        match golden[i].cmp(&actual[j]) {
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => {
                missing.push(golden[i].clone());
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                extra.push(actual[j].clone());
                j += 1;
            }
        }
    }
    while i < golden.len() {
        missing.push(golden[i].clone());
        i += 1;
    }
    while j < actual.len() {
        extra.push(actual[j].clone());
        j += 1;
    }
    (missing, extra)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cs(items: &[&str]) -> Vec<CompactString> {
        let mut v: Vec<CompactString> = items.iter().map(|s| (*s).into()).collect();
        v.sort();
        v
    }

    #[test]
    fn identical_sets_have_no_diff() {
        let g = cs(&["useSWR"]);
        let a = cs(&["useSWR"]);
        let (m, e) = diff_sets(&g, &a);
        assert!(m.is_empty());
        assert!(e.is_empty());
    }

    #[test]
    fn missing_call_detected() {
        let g = cs(&["useSWR", "axios"]);
        let a = cs(&["useSWR"]);
        let (m, e) = diff_sets(&g, &a);
        assert_eq!(m, cs(&["axios"]));
        assert!(e.is_empty());
    }

    #[test]
    fn extra_call_detected() {
        let g = cs(&["useSWR"]);
        let a = cs(&["useSWR", "fetch"]);
        let (m, e) = diff_sets(&g, &a);
        assert!(m.is_empty());
        assert_eq!(e, cs(&["fetch"]));
    }

    #[test]
    fn both_missing_and_extra() {
        let g = cs(&["useSWR"]);
        let a = cs(&["fetch"]);
        let (m, e) = diff_sets(&g, &a);
        assert_eq!(m, cs(&["useSWR"]));
        assert_eq!(e, cs(&["fetch"]));
    }

    fn golden_match(file: &str, sym: &str, overall: f64) -> GoldenMatch {
        GoldenMatch {
            golden: GoldenSelector { file: file.into(), symbol: sym.into() },
            similarity: SimilarityScore {
                overall,
                shape: overall,
                calls: overall,
                imports: overall,
                signature: overall,
            },
        }
    }

    #[test]
    fn pick_best_single_golden_returns_it() {
        let m = golden_match("a.ts", "a", 0.42);
        let (best, sorted) = pick_best(vec![m.clone()]);
        assert_eq!(best, m);
        assert_eq!(sorted, vec![m]);
    }

    #[test]
    fn pick_best_picks_higher_overall() {
        let lo = golden_match("a.ts", "a", 0.3);
        let hi = golden_match("b.ts", "b", 0.8);
        let (best, sorted) = pick_best(vec![lo.clone(), hi.clone()]);
        assert_eq!(best, hi);
        assert_eq!(sorted, vec![hi, lo]);
    }

    #[test]
    fn pick_best_sorts_three_descending() {
        let a = golden_match("a.ts", "a", 0.3);
        let b = golden_match("b.ts", "b", 0.8);
        let c = golden_match("c.ts", "c", 0.5);
        let (_, sorted) = pick_best(vec![a, b.clone(), c.clone()]);
        assert_eq!(sorted[0].golden.file, "b.ts");
        assert_eq!(sorted[1].golden.file, "c.ts");
        assert_eq!(sorted[2].golden.file, "a.ts");
    }

    #[test]
    #[should_panic(expected = "at least one golden")]
    fn pick_best_panics_on_empty_input() {
        let _ = pick_best(vec![]);
    }
}

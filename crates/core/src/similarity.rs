//! 4-axis similarity score.
//!
//! Decomposes similarity into shape (TSED), calls (Jaccard), imports (Jaccard),
//! and signature (binary) so that `--explain` / `why` can show *what* differs.

use compact_str::CompactString;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimilarityScore {
    pub overall: f64,
    pub shape: f64,
    pub calls: f64,
    pub imports: f64,
    pub signature: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Weights {
    pub shape: f64,
    pub calls: f64,
    pub imports: f64,
    pub signature: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self { shape: 0.6, calls: 0.3, imports: 0.1, signature: 0.0 }
    }
}

/// Jaccard similarity over two sorted slices. Both inputs MUST be sorted.
pub fn jaccard_sorted(a: &[CompactString], b: &[CompactString]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let (mut i, mut j, mut intersect, mut union_n) = (0_usize, 0_usize, 0_usize, 0_usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                intersect += 1;
                union_n += 1;
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => {
                union_n += 1;
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                union_n += 1;
                j += 1;
            }
        }
    }
    union_n += a.len() - i;
    union_n += b.len() - j;
    if union_n == 0 {
        return 1.0;
    }
    intersect as f64 / union_n as f64
}

/// Build a SimilarityScore from per-axis values and weights.
pub fn aggregate(
    shape: f64,
    calls: f64,
    imports: f64,
    signature: f64,
    w: Weights,
) -> SimilarityScore {
    let total_w = w.shape + w.calls + w.imports + w.signature;
    let overall = if total_w > 0.0 {
        (shape * w.shape + calls * w.calls + imports * w.imports + signature * w.signature)
            / total_w
    } else {
        0.0
    };
    SimilarityScore { overall, shape, calls, imports, signature }
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
    fn jaccard_empty_inputs() {
        assert!((jaccard_sorted(&[], &[]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_identical_sets() {
        let a = cs(&["useSWR", "axios"]);
        let b = cs(&["useSWR", "axios"]);
        assert!((jaccard_sorted(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_disjoint_sets() {
        let a = cs(&["useSWR"]);
        let b = cs(&["axios"]);
        assert!(jaccard_sorted(&a, &b).abs() < 1e-9);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let a = cs(&["useSWR", "axios"]);
        let b = cs(&["useSWR", "fetch"]);
        let j = jaccard_sorted(&a, &b);
        assert!((j - 1.0 / 3.0).abs() < 1e-9, "got {j}");
    }

    #[test]
    fn aggregate_uses_weights() {
        let s = aggregate(1.0, 0.0, 0.0, 0.0, Weights::default());
        assert!((s.overall - 0.6).abs() < 1e-9);
        assert!((s.shape - 1.0).abs() < 1e-9);
    }
}

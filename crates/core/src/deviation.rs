//! Deviation differences: what's missing/extra vs the golden.

use compact_str::CompactString;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Differences {
    pub missing_calls: Vec<CompactString>,
    pub extra_calls: Vec<CompactString>,
    pub missing_imports: Vec<CompactString>,
    pub extra_imports: Vec<CompactString>,
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
}

//! Compiled rule used by the matcher and pipeline.
//!
//! Constructed by `lc-io::config` from JSON. `lc-core` keeps no I/O,
//! so this type only knows about already-compiled glob sets.

use std::path::Path;

use globset::GlobSet;

#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub goldens: Vec<GoldenSelector>,
    pub apply_to: GlobSet,
    pub ignore: GlobSet,
    pub threshold: Option<f64>,
    pub disabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GoldenSelector {
    pub file: String,
    pub symbol: String,
}

impl Rule {
    /// Returns true if `path` matches `apply_to` and is not excluded by `ignore`.
    /// Disabled rules never match.
    pub fn matches(&self, path: &Path) -> bool {
        if self.disabled {
            return false;
        }
        if !self.apply_to.is_match(path) {
            return false;
        }
        if self.ignore.is_match(path) {
            return false;
        }
        true
    }
}

//! Pure rule × file matching.

use std::path::Path;

use crate::rule::Rule;

/// Returns the subset of `rules` that match `path`.
///
/// A rule matches when its `applyTo` glob accepts the path and its `ignore`
/// glob does not. Disabled rules are skipped.
pub fn matching_rules<'r>(path: &Path, rules: &'r [Rule]) -> Vec<&'r Rule> {
    rules.iter().filter(|r| r.matches(path)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use globset::{Glob, GlobSet, GlobSetBuilder};

    fn glob_set(patterns: &[&str]) -> GlobSet {
        let mut b = GlobSetBuilder::new();
        for p in patterns {
            b.add(Glob::new(p).expect("valid glob"));
        }
        b.build().expect("build glob set")
    }

    fn rule(id: &str, apply: &[&str], ignore: &[&str], disabled: bool) -> Rule {
        Rule {
            id: id.into(),
            goldens: vec![],
            apply_to: glob_set(apply),
            ignore: glob_set(ignore),
            threshold: None,
            disabled,
        }
    }

    #[test]
    fn returns_rule_when_apply_to_matches() {
        let rules = vec![rule("repos", &["src/repos/**/*.ts"], &[], false)];
        let m = matching_rules(Path::new("src/repos/user.ts"), &rules);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, "repos");
    }

    #[test]
    fn empty_when_no_apply_to_matches() {
        let rules = vec![rule("repos", &["src/repos/**/*.ts"], &[], false)];
        let m = matching_rules(Path::new("src/components/Button.tsx"), &rules);
        assert!(m.is_empty());
    }

    #[test]
    fn ignore_overrides_apply_to() {
        let rules = vec![rule(
            "repos",
            &["src/repos/**/*.ts"],
            &["src/repos/legacy/**"],
            false,
        )];
        let m = matching_rules(Path::new("src/repos/legacy/old.ts"), &rules);
        assert!(m.is_empty(), "ignore should suppress");
        let m = matching_rules(Path::new("src/repos/user.ts"), &rules);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn multiple_rules_can_match_same_path() {
        let rules = vec![
            rule("ts", &["**/*.ts"], &[], false),
            rule("repos", &["src/repos/**/*.ts"], &[], false),
        ];
        let m = matching_rules(Path::new("src/repos/user.ts"), &rules);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn disabled_rule_never_matches() {
        let rules = vec![rule("repos", &["**/*.ts"], &[], true)];
        let m = matching_rules(Path::new("src/foo.ts"), &rules);
        assert!(m.is_empty());
    }

    #[test]
    fn multiple_apply_to_patterns_are_or() {
        let rules = vec![rule(
            "many",
            &["src/a/**/*.ts", "src/b/**/*.ts"],
            &[],
            false,
        )];
        assert_eq!(matching_rules(Path::new("src/a/x.ts"), &rules).len(), 1);
        assert_eq!(matching_rules(Path::new("src/b/y.ts"), &rules).len(), 1);
        assert!(matching_rules(Path::new("src/c/z.ts"), &rules).is_empty());
    }
}

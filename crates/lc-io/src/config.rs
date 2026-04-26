//! `.layer-conform.json` schema and loader.
//!
//! The on-disk schema accepts polymorphic shapes for ergonomics
//! (`golden` as string | object | array; `applyTo`/`ignore` as string | array)
//! but the in-memory `Config` is normalized so callers see one shape.

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("invalid config: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub version: u32,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub id: String,
    pub goldens: Vec<GoldenSelector>,
    pub apply_to: Vec<String>,
    pub ignore: Vec<String>,
    pub threshold: Option<f64>,
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GoldenSelector {
    pub file: String,
    pub symbol: String,
}

pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path)?;
    from_str(&text)
}

pub fn from_str(text: &str) -> Result<Config, ConfigError> {
    let raw: RawConfig = serde_json::from_str(text)?;
    raw.try_into()
}

// --- on-disk representation ---------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    version: u32,
    rules: Vec<RawRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawRule {
    id: String,
    golden: RawGolden,
    apply_to: StringOrVec,
    #[serde(default)]
    ignore: Option<StringOrVec>,
    #[serde(default)]
    threshold: Option<f64>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawGolden {
    Single(RawGoldenItem),
    Multi(Vec<RawGoldenItem>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawGoldenItem {
    Shorthand(String),
    Object {
        file: String,
        symbol: String,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    One(String),
    Many(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(s) => vec![s],
            Self::Many(v) => v,
        }
    }
}

// --- validation ---------------------------------------------------------------

impl TryFrom<RawConfig> for Config {
    type Error = ConfigError;

    fn try_from(raw: RawConfig) -> Result<Self, Self::Error> {
        if raw.version != 1 {
            return Err(ConfigError::Validation(format!(
                "unsupported config version {} (expected 1)",
                raw.version
            )));
        }

        let mut seen_ids: HashSet<String> = HashSet::with_capacity(raw.rules.len());
        let mut rules = Vec::with_capacity(raw.rules.len());
        for raw_rule in raw.rules {
            let rule = Rule::try_from(raw_rule)?;
            if !seen_ids.insert(rule.id.clone()) {
                return Err(ConfigError::Validation(format!(
                    "duplicate rule id: {}",
                    rule.id
                )));
            }
            rules.push(rule);
        }

        Ok(Self { version: raw.version, rules })
    }
}

impl TryFrom<RawRule> for Rule {
    type Error = ConfigError;

    fn try_from(raw: RawRule) -> Result<Self, Self::Error> {
        if raw.id.trim().is_empty() {
            return Err(ConfigError::Validation("rule id must be non-empty".into()));
        }

        let goldens = match raw.golden {
            RawGolden::Single(item) => vec![GoldenSelector::try_from(item)?],
            RawGolden::Multi(items) => {
                if items.is_empty() {
                    return Err(ConfigError::Validation(format!(
                        "rule `{}` has empty golden array",
                        raw.id
                    )));
                }
                items
                    .into_iter()
                    .map(GoldenSelector::try_from)
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        Ok(Self {
            id: raw.id,
            goldens,
            apply_to: raw.apply_to.into_vec(),
            ignore: raw.ignore.map(StringOrVec::into_vec).unwrap_or_default(),
            threshold: raw.threshold,
            disabled: raw.disabled,
        })
    }
}

impl TryFrom<RawGoldenItem> for GoldenSelector {
    type Error = ConfigError;

    fn try_from(item: RawGoldenItem) -> Result<Self, Self::Error> {
        match item {
            RawGoldenItem::Shorthand(s) => parse_shorthand(&s),
            RawGoldenItem::Object { file, symbol } => {
                if file.is_empty() || symbol.is_empty() {
                    Err(ConfigError::Validation(format!(
                        "golden object has empty field (file={file:?}, symbol={symbol:?})",
                    )))
                } else {
                    Ok(Self { file, symbol })
                }
            }
        }
    }
}

fn parse_shorthand(s: &str) -> Result<GoldenSelector, ConfigError> {
    let (file, symbol) = s.rsplit_once(':').ok_or_else(|| {
        ConfigError::Validation(format!(
            "golden shorthand must be \"<file>:<symbol>\", got {s:?}",
        ))
    })?;
    if file.is_empty() || symbol.is_empty() {
        return Err(ConfigError::Validation(format!(
            "golden shorthand has empty part: {s:?}"
        )));
    }
    Ok(GoldenSelector { file: file.to_string(), symbol: symbol.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Config {
        from_str(s).expect("expected valid config")
    }

    #[test]
    fn parses_minimal_config_with_string_golden_and_string_apply_to() {
        let cfg = parse(
            r#"{
                "version": 1,
                "rules": [{
                    "id": "repos",
                    "golden": "src/repos/a.ts:a",
                    "applyTo": "src/repos/**/*.ts"
                }]
            }"#,
        );
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.rules.len(), 1);
        let r = &cfg.rules[0];
        assert_eq!(r.id, "repos");
        assert_eq!(r.goldens, vec![GoldenSelector { file: "src/repos/a.ts".into(), symbol: "a".into() }]);
        assert_eq!(r.apply_to, vec!["src/repos/**/*.ts".to_string()]);
        assert!(r.ignore.is_empty());
        assert!(r.threshold.is_none());
        assert!(!r.disabled);
    }

    #[test]
    fn parses_object_form_golden() {
        let cfg = parse(
            r#"{ "version": 1, "rules": [{
                "id": "x",
                "golden": { "file": "a.ts", "symbol": "a" },
                "applyTo": "**/*.ts"
            }]}"#,
        );
        assert_eq!(cfg.rules[0].goldens, vec![GoldenSelector { file: "a.ts".into(), symbol: "a".into() }]);
    }

    #[test]
    fn parses_array_of_mixed_golden_forms() {
        let cfg = parse(
            r#"{ "version": 1, "rules": [{
                "id": "x",
                "golden": [
                    "a.ts:a",
                    { "file": "b.ts", "symbol": "b" }
                ],
                "applyTo": "**/*.ts"
            }]}"#,
        );
        assert_eq!(
            cfg.rules[0].goldens,
            vec![
                GoldenSelector { file: "a.ts".into(), symbol: "a".into() },
                GoldenSelector { file: "b.ts".into(), symbol: "b".into() },
            ]
        );
    }

    #[test]
    fn parses_apply_to_and_ignore_as_arrays() {
        let cfg = parse(
            r#"{ "version": 1, "rules": [{
                "id": "x",
                "golden": "a.ts:a",
                "applyTo": ["src/**/*.ts", "lib/**/*.ts"],
                "ignore": ["**/legacy/**", "**/*.spec.ts"]
            }]}"#,
        );
        assert_eq!(cfg.rules[0].apply_to.len(), 2);
        assert_eq!(cfg.rules[0].ignore.len(), 2);
    }

    #[test]
    fn parses_threshold_and_disabled() {
        let cfg = parse(
            r#"{ "version": 1, "rules": [{
                "id": "x",
                "golden": "a.ts:a",
                "applyTo": "**/*.ts",
                "threshold": 0.85,
                "disabled": true
            }]}"#,
        );
        assert_eq!(cfg.rules[0].threshold, Some(0.85));
        assert!(cfg.rules[0].disabled);
    }

    #[test]
    fn rejects_duplicate_rule_id() {
        let err = from_str(
            r#"{ "version": 1, "rules": [
                {"id": "x", "golden": "a.ts:a", "applyTo": "**/*.ts"},
                {"id": "x", "golden": "b.ts:b", "applyTo": "**/*.ts"}
            ]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(ref m) if m.contains("duplicate")), "got: {err:?}");
    }

    #[test]
    fn rejects_missing_rule_id() {
        // Missing `id` field is a parse error (serde missing field)
        let err = from_str(
            r#"{ "version": 1, "rules": [{"golden": "a.ts:a", "applyTo": "**/*.ts"}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)), "got: {err:?}");
    }

    #[test]
    fn rejects_empty_rule_id() {
        let err = from_str(
            r#"{ "version": 1, "rules": [{"id": "", "golden": "a.ts:a", "applyTo": "**/*.ts"}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let err = from_str(
            r#"{ "version": 1, "rules": [], "unknownField": 1 }"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn rejects_unknown_rule_field() {
        let err = from_str(
            r#"{ "version": 1, "rules": [
                {"id": "x", "golden": "a.ts:a", "applyTo": "**/*.ts", "huh": 1}
            ]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn rejects_unsupported_version() {
        let err = from_str(r#"{ "version": 99, "rules": [] }"#).unwrap_err();
        assert!(matches!(err, ConfigError::Validation(ref m) if m.contains("version")), "got: {err:?}");
    }

    #[test]
    fn rejects_golden_shorthand_without_colon() {
        let err = from_str(
            r#"{ "version": 1, "rules": [{"id": "x", "golden": "no-colon", "applyTo": "**"}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
    }
}

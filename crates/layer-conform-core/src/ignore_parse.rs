//! Detect `layer-conform-ignore` directives in a function's leading comments.
//!
//! Pure (no I/O, no language deps). The language adapter (`layer-conform-ts`) collects
//! the comments preceding a function and feeds them in. The detector decides
//! whether a directive is present and extracts the optional reason.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentKind {
    /// `// ...`
    Line,
    /// `/* ... */`
    Block,
    /// `/** ... */` (`JSDoc`)
    Doc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentToken {
    /// Comment body **without** the surrounding `//`, `/*`, `*/` markers.
    pub text: String,
    pub kind: CommentKind,
    /// 1-indexed line where the comment ends.
    pub end_line: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IgnoreDirective {
    /// `None` when the reason was omitted (caller should warn).
    pub reason: Option<String>,
}

/// Maximum lines the directive comment may sit above the function start.
/// Spec §7: directive must appear in the immediately preceding 1-3 lines.
const MAX_LINE_GAP: u32 = 3;

const MARKER: &str = "layer-conform-ignore";
const DOC_MARKER: &str = "@layer-conform-ignore";

/// Scan `comments` for an ignore directive that applies to a function starting
/// at `function_start_line` (1-indexed).
///
/// If multiple comments contain the marker, the one closest to the function wins.
/// Comments outside the 3-line window are ignored.
pub fn parse_directive(
    comments: &[CommentToken],
    function_start_line: u32,
) -> Option<IgnoreDirective> {
    let mut best: Option<(u32, IgnoreDirective)> = None;
    for c in comments {
        if c.end_line >= function_start_line {
            continue;
        }
        let gap = function_start_line - c.end_line;
        if gap == 0 || gap > MAX_LINE_GAP {
            continue;
        }
        let Some(reason) = detect(&c.text, c.kind) else { continue };
        let directive = IgnoreDirective { reason };
        match best {
            Some((prev_gap, _)) if prev_gap <= gap => {}
            _ => best = Some((gap, directive)),
        }
    }
    best.map(|(_, d)| d)
}

fn detect(text: &str, kind: CommentKind) -> Option<Option<String>> {
    if matches!(kind, CommentKind::Doc) {
        if let Some(reason) = find_marker(text, DOC_MARKER) {
            return Some(reason);
        }
    }
    find_marker(text, MARKER)
}

/// Returns `Some(reason)` when `marker` is found at a word boundary, else `None`.
/// Inner `Option<String>` is the reason text (None when omitted).
fn find_marker(text: &str, marker: &str) -> Option<Option<String>> {
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find(marker) {
        let pos = search_from + rel;
        let before_ok = pos == 0
            || !text.as_bytes()[pos - 1].is_ascii_alphanumeric()
                && text.as_bytes()[pos - 1] != b'_';
        let after_idx = pos + marker.len();
        let next_byte = text.as_bytes().get(after_idx).copied();
        let after_ok = match next_byte {
            None => true,
            Some(b) => !(b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        };
        if before_ok && after_ok {
            let after = &text[after_idx..];
            return Some(extract_reason(after));
        }
        search_from = pos + marker.len();
    }
    None
}

fn extract_reason(after: &str) -> Option<String> {
    let trimmed = after.trim_start_matches([' ', '\t']);
    let Some(rest) = trimmed.strip_prefix(':') else { return None };
    let line = rest.lines().next().unwrap_or("");
    let cleaned = line.trim().trim_end_matches('*').trim().trim_end_matches('/').trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_at(line: u32, text: &str) -> CommentToken {
        CommentToken { text: text.into(), kind: CommentKind::Line, end_line: line }
    }
    fn block_at(line: u32, text: &str) -> CommentToken {
        CommentToken { text: text.into(), kind: CommentKind::Block, end_line: line }
    }
    fn doc_at(line: u32, text: &str) -> CommentToken {
        CommentToken { text: text.into(), kind: CommentKind::Doc, end_line: line }
    }

    #[test]
    fn line_comment_with_reason_is_detected() {
        let c = line_at(9, " layer-conform-ignore: testing only ");
        let d = parse_directive(&[c], 10).expect("directive");
        assert_eq!(d.reason.as_deref(), Some("testing only"));
    }

    #[test]
    fn line_comment_without_reason_returns_none_reason() {
        let c = line_at(9, " layer-conform-ignore");
        let d = parse_directive(&[c], 10).expect("directive");
        assert!(d.reason.is_none());
    }

    #[test]
    fn unrelated_comment_returns_none() {
        let c = line_at(9, " just some note");
        assert!(parse_directive(&[c], 10).is_none());
    }

    #[test]
    fn block_comment_with_reason_is_detected() {
        let c = block_at(9, " layer-conform-ignore: workaround ");
        let d = parse_directive(&[c], 10).expect("directive");
        assert_eq!(d.reason.as_deref(), Some("workaround"));
    }

    #[test]
    fn doc_comment_with_at_marker_is_detected() {
        let c = doc_at(9, "* @layer-conform-ignore: legacy adapter\n* more text");
        let d = parse_directive(&[c], 10).expect("directive");
        assert_eq!(d.reason.as_deref(), Some("legacy adapter"));
    }

    #[test]
    fn doc_comment_with_bare_marker_is_detected() {
        let c = doc_at(9, "* layer-conform-ignore: needs cleanup\n");
        let d = parse_directive(&[c], 10).expect("directive");
        assert_eq!(d.reason.as_deref(), Some("needs cleanup"));
    }

    #[test]
    fn directive_three_lines_above_is_in_range() {
        let c = line_at(7, " layer-conform-ignore: ok ");
        assert!(parse_directive(&[c], 10).is_some());
    }

    #[test]
    fn directive_four_lines_above_is_out_of_range() {
        let c = line_at(6, " layer-conform-ignore: too far ");
        assert!(parse_directive(&[c], 10).is_none());
    }

    #[test]
    fn comment_below_function_is_ignored() {
        let c = line_at(11, " layer-conform-ignore: below ");
        assert!(parse_directive(&[c], 10).is_none());
    }

    #[test]
    fn closest_directive_wins_when_multiple() {
        let far = line_at(7, " layer-conform-ignore: far ");
        let near = line_at(9, " layer-conform-ignore: near ");
        let d = parse_directive(&[far, near], 10).expect("directive");
        assert_eq!(d.reason.as_deref(), Some("near"));
    }

    #[test]
    fn marker_substring_does_not_match() {
        let c = line_at(9, " not-layer-conform-ignored stuff ");
        assert!(parse_directive(&[c], 10).is_none());
    }
}

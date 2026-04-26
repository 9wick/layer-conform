//! Anti-Corruption Layer for `oxc_parser` API.
//! All `oxc_*` types are confined to this module.

use oxc_allocator::Allocator;
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

/// Parse a TypeScript / JSX source. Allocator is owned by the caller and
/// must outlive the returned `Program<'a>`.
pub fn parse<'a>(allocator: &'a Allocator, source: &'a str) -> ParserReturn<'a> {
    let source_type = SourceType::default()
        .with_typescript(true)
        .with_jsx(true)
        .with_module(true);
    Parser::new(allocator, source, source_type).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_source() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "");
        assert!(ret.errors.is_empty());
        assert_eq!(ret.program.body.len(), 0);
    }

    #[test]
    fn parses_function_declaration() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function foo() {}");
        assert!(ret.errors.is_empty());
        assert_eq!(ret.program.body.len(), 1);
    }

    #[test]
    fn parses_typescript_syntax() {
        let alloc = Allocator::default();
        let ret = parse(&alloc, "function foo(x: number): string { return ''; }");
        assert!(ret.errors.is_empty());
    }
}

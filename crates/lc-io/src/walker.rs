//! Walk source files under a root, respecting `.gitignore`.
//!
//! Returned paths are absolute. Caller is responsible for converting to a
//! root-relative form before matching against rules.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

const SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// Walk every TS/JS source file under `root`, skipping anything ignored by
/// `.gitignore` / `.ignore` rules.
pub fn walk_source_files(root: &Path) -> Vec<PathBuf> {
    WalkBuilder::new(root)
        .hidden(false)
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter(|e| has_source_extension(e.path()))
        .map(ignore::DirEntry::into_path)
        .collect()
}

fn has_source_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some(ext) if SOURCE_EXTENSIONS.contains(&ext)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn walks_source_files_only() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.ts"), "").unwrap();
        fs::write(dir.path().join("b.tsx"), "").unwrap();
        fs::write(dir.path().join("c.md"), "").unwrap();
        fs::write(dir.path().join("d.json"), "").unwrap();
        let mut v: Vec<String> = walk_source_files(dir.path())
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        v.sort();
        assert_eq!(v, vec!["a.ts", "b.tsx"]);
    }

    #[test]
    fn skips_files_listed_in_dot_ignore() {
        // Use `.ignore` (always honored) instead of `.gitignore` which `ignore`
        // only consults inside a git work tree.
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".ignore"), "skip.ts\n").unwrap();
        fs::write(dir.path().join("keep.ts"), "").unwrap();
        fs::write(dir.path().join("skip.ts"), "").unwrap();
        let names: Vec<String> = walk_source_files(dir.path())
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"keep.ts".to_string()));
        assert!(!names.contains(&"skip.ts".to_string()));
    }
}

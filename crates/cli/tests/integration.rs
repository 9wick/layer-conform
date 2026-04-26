//! End-to-end integration tests for the `layer-conform` binary.

use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

fn write(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
}

#[test]
fn check_reports_overall_one_for_identical_function() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.ts");
    write(
        &file,
        "import { useSWR } from 'swr';\nfunction useFoo() { return useSWR('/x'); }\n",
    );

    Command::cargo_bin("layer-conform")
        .unwrap()
        .args([
            "check",
            "--file",
            file.to_str().unwrap(),
            "--symbol",
            "useFoo",
            "--golden",
        ])
        .arg(format!("{}:useFoo", file.display()))
        .assert()
        .success()
        .stdout(contains("overall=1.000"));
}

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

#[test]
fn check_reports_missing_and_extra_calls_for_divergent_styles() {
    let dir = tempdir().unwrap();
    let golden = dir.path().join("golden.ts");
    let actual = dir.path().join("actual.ts");
    write(
        &golden,
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(&actual, "function useActual() { return fetch('/x'); }\n");

    Command::cargo_bin("layer-conform")
        .unwrap()
        .args([
            "check",
            "--file",
            actual.to_str().unwrap(),
            "--symbol",
            "useActual",
            "--golden",
        ])
        .arg(format!("{}:useGolden", golden.display()))
        .assert()
        .success()
        .stdout(contains("missing calls"))
        .stdout(contains("useSWR"))
        .stdout(contains("extra calls"))
        .stdout(contains("fetch"));
}

#[test]
fn check_fails_when_symbol_not_found() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.ts");
    write(&file, "function existing() {}\n");

    Command::cargo_bin("layer-conform")
        .unwrap()
        .args([
            "check",
            "--file",
            file.to_str().unwrap(),
            "--symbol",
            "missing_symbol",
            "--golden",
        ])
        .arg(format!("{}:existing", file.display()))
        .assert()
        .failure();
}

#[test]
fn check_handles_class_methods_with_dotted_symbols() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("resolver.ts");
    write(
        &file,
        r#"class FooResolver {
  async list() { return this.svc.findAll(); }
  async one() { return this.svc.findFirst(); }
}
"#,
    );

    Command::cargo_bin("layer-conform")
        .unwrap()
        .args([
            "check",
            "--file",
            file.to_str().unwrap(),
            "--symbol",
            "FooResolver.list",
            "--golden",
        ])
        .arg(format!("{}:FooResolver.one", file.display()))
        .assert()
        .success()
        .stdout(contains("FooResolver.list"))
        .stdout(contains("FooResolver.one"));
}

#[test]
fn check_walker_collects_calls_inside_variable_decl() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("a.ts");
    write(
        &file,
        "function f() { const x = useSWR('/u'); return x; }\n",
    );

    Command::cargo_bin("layer-conform")
        .unwrap()
        .args([
            "check",
            "--file",
            file.to_str().unwrap(),
            "--symbol",
            "f",
            "--golden",
        ])
        .arg(format!("{}:f", file.display()))
        .assert()
        .success()
        // self-compare → all 1.000
        .stdout(contains("calls=1.000"));
}

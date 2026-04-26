//! End-to-end integration tests for the `layer-conform` binary.
//!
//! Every test sets up a tempdir with a `.layer-conform.json` plus source
//! fixtures, then runs the CLI with the tempdir as its working directory.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn cli(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("layer-conform").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

#[test]
fn no_deviations_when_actual_is_identical_to_golden() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(
        &dir.path().join("src/copy.ts"),
        "import { useSWR } from 'swr';\nfunction useCopy() { return useSWR('/y'); }\n",
    );

    cli(&dir)
        .assert()
        .success()
        .stdout(contains("No deviations"));
}

#[test]
fn deviation_is_reported_and_exit_code_is_one() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(
        &dir.path().join("src/divergent.ts"),
        "function useDiv() { return fetch('/y'); }\n",
    );

    cli(&dir)
        .assert()
        .failure()
        .stdout(contains("DEVIATION"))
        .stdout(contains("src/divergent.ts"))
        .stdout(contains("missing calls"))
        .stdout(contains("useSWR"))
        .stdout(contains("extra calls"))
        .stdout(contains("fetch"));
}

#[test]
fn multi_golden_picks_max_similarity() {
    let dir = tempdir();
    // Two goldens: one returns useSWR, one returns useFetch.
    // Actual uses useFetch — should match golden2 (higher similarity).
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "data",
            "golden": [
                "src/g1.ts:useG1",
                "src/g2.ts:useG2"
            ],
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/g1.ts"),
        "function useG1() { return useSWR('/a'); }\n",
    );
    write(
        &dir.path().join("src/g2.ts"),
        "function useG2() { return useFetch('/b'); }\n",
    );
    write(
        &dir.path().join("src/probe.ts"),
        "function probe() { return useFetch('/c'); }\n",
    );

    cli(&dir)
        .assert()
        // probe vs g2 = matches (calls=1.0, signature=1.0); should be conform.
        .success()
        .stdout(contains("No deviations"));
}

#[test]
fn ignore_directive_skips_function() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(
        &dir.path().join("src/legacy.ts"),
        "// layer-conform-ignore: legacy adapter\nfunction useLegacy() { return fetch('/y'); }\n",
    );

    cli(&dir)
        .assert()
        .success()
        .stdout(contains("No deviations"));
}

#[test]
fn json_output_contains_deviation_fields() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(
        &dir.path().join("src/diff.ts"),
        "function useDiff() { return fetch('/y'); }\n",
    );

    cli(&dir)
        .args(["--json"])
        .assert()
        .failure()
        .stdout(contains("\"version\": 1"))
        .stdout(contains("\"deviations\""))
        .stdout(contains("\"rule_id\": \"repos\""))
        .stdout(contains("\"matched_golden\""));
}

#[test]
fn no_color_strips_ansi_escape_codes() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(
        &dir.path().join("src/diff.ts"),
        "function useDiff() { return fetch('/y'); }\n",
    );

    let assert = cli(&dir).args(["--no-color"]).assert().failure();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains('\x1b'), "stdout should not contain ANSI escape codes");
}

#[test]
fn threshold_override_can_flip_a_deviation_to_conform() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        // Default threshold 0.7. Override to 0.0 — every function conforms.
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(
        &dir.path().join("src/diff.ts"),
        "function useDiff() { return fetch('/y'); }\n",
    );

    cli(&dir)
        .args(["--threshold", "0.0"])
        .assert()
        .success()
        .stdout(contains("No deviations"));
}

#[test]
fn init_writes_starter_config() {
    let dir = tempdir();
    cli(&dir).args(["init"]).assert().success();
    let path = dir.path().join(".layer-conform.json");
    assert!(path.exists());
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("\"version\""));
    assert!(content.contains("\"rules\""));
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let dir = tempdir();
    write(&dir.path().join(".layer-conform.json"), "{}");
    cli(&dir).args(["init"]).assert().failure();
    // unchanged
    assert_eq!(fs::read_to_string(dir.path().join(".layer-conform.json")).unwrap(), "{}");
}

#[test]
fn init_force_overwrites_existing() {
    let dir = tempdir();
    write(&dir.path().join(".layer-conform.json"), "{}");
    cli(&dir).args(["init", "--force"]).assert().success();
    let content = fs::read_to_string(dir.path().join(".layer-conform.json")).unwrap();
    assert!(content.contains("\"version\""));
}

#[test]
fn why_shows_per_rule_per_golden_score() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(
        &dir.path().join("src/probe.ts"),
        "function useProbe() { return fetch('/y'); }\n",
    );

    cli(&dir)
        .args(["why", "src/probe.ts"])
        .assert()
        .success()
        .stdout(contains("rule `repos`"))
        .stdout(contains("vs golden src/golden.ts:useGolden"))
        .stdout(contains("DEVIATION"));
}

#[test]
fn why_for_unmatched_file_says_no_rule_matches() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(&dir.path().join("lib/other.ts"), "function f() {}\n");

    cli(&dir)
        .args(["why", "lib/other.ts"])
        .assert()
        .success()
        .stdout(contains("no rule matches"));
}

#[test]
fn explain_filters_to_one_file() {
    let dir = tempdir();
    write(
        &dir.path().join(".layer-conform.json"),
        r#"{ "version": 1, "rules": [{
            "id": "repos",
            "golden": "src/golden.ts:useGolden",
            "applyTo": "src/**/*.ts"
        }]}"#,
    );
    write(
        &dir.path().join("src/golden.ts"),
        "import { useSWR } from 'swr';\nfunction useGolden() { return useSWR('/x'); }\n",
    );
    write(&dir.path().join("src/a.ts"), "function a() { return fetch('/x'); }\n");
    write(&dir.path().join("src/b.ts"), "function b() { return fetch('/y'); }\n");

    let out = cli(&dir)
        .args(["check", "--explain", "src/a.ts"])
        .assert()
        .failure();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("src/a.ts"));
    assert!(!stdout.contains("src/b.ts"), "explain should hide other files: {stdout}");
}

fn tempdir() -> TempDir {
    tempfile::tempdir().unwrap()
}

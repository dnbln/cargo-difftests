// tests/tests.rs

use cargo_difftests_sample_project::*;

#[derive(serde::Serialize, Clone)]
struct ExtraArgs {
    pkg_name: String,
    crate_name: String,
    bin_name: Option<String>,
    test_name: String,
}

#[must_use]
fn setup_difftests(test_name: &str) {
    // the temporary directory where we will store everything we need.
    // this should be passed to various `cargo difftests` subcommands as the
    // `--dir` option.
    cargo_difftests_testclient::write_desc(ExtraArgs {
        pkg_name: env!("CARGO_PKG_NAME").to_string(),
        crate_name: env!("CARGO_CRATE_NAME").to_string(),
        bin_name: option_env!("CARGO_BIN_NAME").map(ToString::to_string),
        test_name: test_name.to_string(),
    })
    .unwrap();
}

#[test]
fn test_add() {
    assert_eq!(add(1, 2), 3);
}

#[test]
fn test_sub() {
    assert_eq!(sub(3, 2), 1);
}

#[test]
fn test_mul() {
    assert_eq!(mul(2, 3), 6);
}

#[test]
fn test_div() {
    assert_eq!(div(6, 3), Some(2));
}

#[test]
fn test_div_2() {
    assert_eq!(div(6, 0), None);
}

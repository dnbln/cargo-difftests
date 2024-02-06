// tests/tests.rs

use {{crate_name}}::*;

use cargo_difftests_testclient::DifftestsEnv;

#[derive(serde::Serialize, Clone)]
struct ExtraArgs {
    pkg_name: String,
    crate_name: String,
    bin_name: Option<String>,
    test_name: String,
    group_name: String,
}

#[must_use]
fn setup_difftests(test_name: &str) -> DifftestsEnv {
    // the temporary directory where we will store everything we need.
    // this should be passed to various `cargo difftests` subcommands as the
    // `--dir` option.
    let tmpdir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("cargo-difftests")
        .join(test_name);
    let difftests_env = cargo_difftests_testclient::init(
        cargo_difftests_testclient::TestDesc::<ExtraArgs> {
            // a "description" of the test.
            // cargo-difftests needs the binary path for analysis
            bin_path: std::env::current_exe().unwrap(),
            // the extra fields that you might need to identify the test.
            //
            // it is your job to use
            // the data in here to identify the test
            // and rerun it if needed.
            // you can use any type you want, but it has to
            // implement `serde::Serialize`
            extra: ExtraArgs {
                pkg_name: env!("CARGO_PKG_NAME").to_string(),
                crate_name: env!("CARGO_CRATE_NAME").to_string(),
                bin_name: option_env!("CARGO_BIN_NAME").map(ToString::to_string),
                test_name: test_name.to_string(),
            },
        },
        &tmpdir,
    )
    .unwrap();
    // the difftests_env is important, because its Drop impl has
    // some cleanup to do (including actually writing the profile).
    //
    // if spawning children, it is also needed to
    // pass some environment variables to them, like this:
    //
    // cmd.envs(difftests_env.env_for_children());
    difftests_env
}

#[cargo_difftests_testclient::test]
fn test_add(_: &DifftestsEnv) {
    assert_eq!(add(1, 2), 3);
}

#[cargo_difftests_testclient::test]
fn test_sub(_: &DifftestsEnv) {
    assert_eq!(sub(3, 2), 1);
}

#[cargo_difftests_testclient::test]
fn test_mul(_: &DifftestsEnv) {
    assert_eq!(mul(2, 3), 6);
}

#[cargo_difftests_testclient::test]
fn test_div(_: &DifftestsEnv) {
    assert_eq!(div(6, 3), Some(2));
}

#[cargo_difftests_testclient::test]
fn test_div_2(_: &DifftestsEnv) {
    assert_eq!(div(6, 0), None);
}

// tests/tests.rs

use cargo_difftests_sample_project::*;

#[cfg(cargo_difftests)]
type DifftestsEnv = cargo_difftests_testclient::DifftestsEnv;

#[cfg(not(cargo_difftests))]
type DifftestsEnv = ();

#[derive(serde::Serialize, Clone)]
struct ExtraArgs {
    pkg_name: String,
    crate_name: String,
    bin_name: Option<String>,
    test_name: String,
    group_name: String,
}

#[must_use]
fn setup_difftests(group: &str, test_name: &str) -> DifftestsEnv {
    #[cfg(cargo_difftests)] // the cargo_difftests_testclient crate is empty
    // without this cfg
    {
        // the temporary directory where we will store everything we need.
        // this should be passed to various `cargo difftests` subcommands as the
        // `--dir` option.
        let tmpdir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("cargo-difftests")
            .join(group)
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
                    group_name: group.to_string(),
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
}

#[test]
fn test_add() {
    let _env = setup_difftests("simple", "test_add");
    std::thread::sleep(std::time::Duration::from_millis(400));
    assert_eq!(add(1, 2), 3);
}

#[test]
fn test_sub() {
    let _env = setup_difftests("simple", "test_sub");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(sub(3, 2), 1);
}

#[test]
fn test_mul() {
    let _env = setup_difftests("advanced", "test_mul");
    std::thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(mul(2, 3), 6);
}

#[test]
fn test_div() {
    let _env = setup_difftests("advanced", "test_div");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(div(6, 3), Some(2));
}

#[test]
fn test_div_2() {
    let _env = setup_difftests("advanced", "test_div_2");
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(div(6, 0), None);
}

// tests/tests.rs

use cargo_difftests_sample_project::*;

fn setup_difftests(group: &str, test_name: &str) {
  #[cfg(cargo_difftests)] // the cargo_difftests_testclient crate is empty 
  // without this cfg
  {
    // the temporary directory where we will store everything we need.
    // this should be passed to various `cargo difftests` subcommands as the 
    // `--dir` option.
    let tmpdir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("cargo-difftests").join(group).join(test_name);
    let difftests_env = cargo_difftests_testclient::init(
      cargo_difftests_testclient::TestDesc {
        // a "description" of the test.
        // cargo-difftests doesn't care about what you put here 
        // (except for the bin_path field) but it is your job to use
        // the data in here to identify the test
        // and rerun it if needed.
        // those fields are here to guide you, but you can add any other
        // fields you might need (see the `other_fields` field below)
        pkg_name: env!("CARGO_PKG_NAME").to_string(),
        crate_name: env!("CARGO_CRATE_NAME").to_string(),
        bin_name: option_env!("CARGO_BIN_NAME").map(ToString::to_string),
        bin_path: std::env::current_exe().unwrap(),
        test_name: test_name.to_string(),
        other_fields: std::collections::HashMap::new(), // any other 
        // fields you might want to add, to identify the test.
        // (the map is of type HashMap<String, String>)
      },
      &tmpdir,
    ).unwrap();
    // right now, the difftests_env is not used, but if 
    // spawning children, it is needed to pass some environment variables to
    // them, like this:
    //
    // cmd.envs(difftests_env.env_for_children());
  }
}

#[test]
fn test_add() {
  setup_difftests("simple", "test_add");
  assert_eq!(add(1, 2), 3);
}

#[test]
fn test_sub() {
  setup_difftests("simple", "test_sub");
  assert_eq!(sub(3, 2), 1);
}

#[test]
fn test_mul() {
  setup_difftests("advanced", "test_mul");
  assert_eq!(mul(2, 3), 6);
}

#[test]
fn test_div() {
  setup_difftests("advanced", "test_div");
  assert_eq!(div(6, 3), Some(2));
}

#[test]
fn test_div_2() {
  setup_difftests("advanced", "test_div_2");
  assert_eq!(div(6, 0), None);
}

#![feature(exit_status_error)]

use cargo_difftests::{cargo_difftests_test_rerunner, test_rerunner_core::TestRerunnerInvocation};

#[derive(serde::Serialize, serde::Deserialize)]
struct TestRerunnerDefaultExtra {
    pkg_name: String,
    test_name: String,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("exit status error: {0}")]
    ExitStatusError(#[from] std::process::ExitStatusError),
}

fn rerunner(invocation: TestRerunnerInvocation) -> Result<(), Error> {
    let profile = std::env::var("CARGO_DIFFTESTS_PROFILE").unwrap_or_else(|_| "difftests".to_owned());

    let extra_cargo_args = std::env::var("CARGO_DIFFTESTS_EXTRA_CARGO_ARGS");
    let extra_cargo_args = extra_cargo_args
        .as_ref()
        .map(|it| it.split(',').collect::<Vec<_>>())
        .unwrap_or_default();
    for test in invocation.tests() {
        let e = test.parse_extra::<TestRerunnerDefaultExtra>()?;
        std::process::Command::new("cargo")
            .args(&["test", "-p", &e.pkg_name, &e.test_name, "--profile", &profile])
            .args(&extra_cargo_args)
            .args(&["--", "--exact"])
            .status()?
            .exit_ok()?;
    }

    Ok(())
}

cargo_difftests_test_rerunner!(rerunner);

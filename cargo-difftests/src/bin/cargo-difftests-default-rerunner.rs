#![feature(exit_status_error)]

use std::io::Read;

use cargo_difftests::{
    cargo_difftests_test_rerunner,
    test_rerunner_core::{TestRerunnerInvocation, TestRunnerInvocationTestCounts},
};

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
    #[error("difftests error: {0}")]
    DifftestsError(#[from] cargo_difftests::DifftestsError),
}

struct FailGuard<'invocation>(TestRunnerInvocationTestCounts<'invocation>);

impl<'invocation> Drop for FailGuard<'invocation> {
    fn drop(&mut self) {
        self.0.fail_if_running().unwrap();
    }
}

fn rerunner(invocation: TestRerunnerInvocation) -> Result<(), Error> {
    let mut counts = FailGuard(invocation.test_counts());
    counts.0.initialize_test_counts(invocation.tests().len())?;

    let profile =
        std::env::var("CARGO_DIFFTESTS_PROFILE").unwrap_or_else(|_| "difftests".to_owned());

    let extra_cargo_args = std::env::var("CARGO_DIFFTESTS_EXTRA_CARGO_ARGS");
    let extra_cargo_args = extra_cargo_args
        .as_ref()
        .map(|it| it.split(',').collect::<Vec<_>>())
        .unwrap_or_default();
    for test in invocation.tests() {
        let e = test.parse_extra::<TestRerunnerDefaultExtra>()?;
        let mut child = std::process::Command::new("cargo")
            .args(&[
                "test",
                "-p",
                &e.pkg_name,
                &e.test_name,
                "--profile",
                &profile,
            ])
            .args(&extra_cargo_args)
            .args(&["--", "--exact"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let mut child_stdout = child.stdout.take().unwrap();
        let mut child_stderr = child.stderr.take().unwrap();

        let r = child.wait()?;

        if r.success() {
            counts.0.inc()?;
        } else {
            counts.0.fail_if_running()?;

            let mut stdout = String::new();
            let mut stderr = String::new();

            child_stdout.read_to_string(&mut stdout)?;
            child_stderr.read_to_string(&mut stderr)?;

            println!("{stdout}");
            eprintln!("{stderr}");

            std::process::exit(1);
        }
    }

    counts.0.test_count_done()?;

    Ok(())
}

cargo_difftests_test_rerunner!(rerunner);

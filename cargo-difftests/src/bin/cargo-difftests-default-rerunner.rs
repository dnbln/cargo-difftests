/*
 *        Copyright (c) 2023-2024 Dinu Blanovschi
 *
 *    Licensed under the Apache License, Version 2.0 (the "License");
 *    you may not use this file except in compliance with the License.
 *    You may obtain a copy of the License at
 *
 *        https://www.apache.org/licenses/LICENSE-2.0
 *
 *    Unless required by applicable law or agreed to in writing, software
 *    distributed under the License is distributed on an "AS IS" BASIS,
 *    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *    See the License for the specific language governing permissions and
 *    limitations under the License.
 */

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

    for test in invocation.tests() {
        let t = counts.0.start_test(test.test_name.clone())?;

        let mut child = std::process::Command::new("cargo")
            .args(&[
                "collect-profiling-data",
                "--filter",
                &test.test_name,
                "--exact",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let mut child_stdout = child.stdout.take().unwrap();
        let mut child_stderr = child.stderr.take().unwrap();

        let r = child.wait()?;

        if r.success() {
            t.test_successful()?;
        } else {
            t.test_failed()?;

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

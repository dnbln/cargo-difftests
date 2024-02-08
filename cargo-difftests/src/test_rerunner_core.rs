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

use std::marker::PhantomData;

use cargo_difftests_core::CoreTestDesc;

use crate::{difftest::TestInfo, AnalyzeAllSingleTest, DifftestsResult};

#[derive(serde::Serialize, serde::Deserialize)]
pub enum State {
    None,
    Running {
        current_test_count: usize,
        total_test_count: usize,
    },
    Done,
    Error,
}

pub struct TestRunnerInvocationTestCounts<'invocation> {
    state: State,
    _pd: PhantomData<&'invocation ()>,
}

impl<'invocation> Drop for TestRunnerInvocationTestCounts<'invocation> {
    fn drop(&mut self) {
        self.test_count_done().unwrap();
    }
}

pub struct TestRunnerInvocationTestCountsTestGuard<'invocation, 'counts> {
    counts: &'counts mut TestRunnerInvocationTestCounts<'invocation>,
    test_name: String,
}

impl<'invocation, 'counts> TestRunnerInvocationTestCountsTestGuard<'invocation, 'counts> {
    pub fn test_successful(self) -> DifftestsResult<()> {
        self.counts.inc()?;
        println!("cargo-difftests-test-successful::{}", self.test_name);
        Ok(())
    }

    pub fn test_failed(self) -> DifftestsResult<()> {
        self.counts.fail_if_running()?;
        println!("cargo-difftests-test-failed::{}", self.test_name);
        Ok(())
    }
}

impl<'invocation> TestRunnerInvocationTestCounts<'invocation> {
    pub fn initialize_test_counts(&mut self, total_tests_to_run: usize) -> DifftestsResult<()> {
        match self.state {
            State::None => {
                self.state = State::Running {
                    current_test_count: 0,
                    total_test_count: total_tests_to_run,
                };

                self.write_test_counts()?;

                Ok(())
            }
            _ => panic!("test counts already initialized"),
        }
    }

    pub fn start_test<'counts>(
        &'counts mut self,
        test_name: String,
    ) -> DifftestsResult<TestRunnerInvocationTestCountsTestGuard<'invocation, 'counts>> {
        match self.state {
            State::Running { .. } => {}
            _ => panic!("test counts not initialized"),
        }

        println!("cargo-difftests-start-test::{}", test_name);

        Ok(TestRunnerInvocationTestCountsTestGuard {
            counts: self,
            test_name,
        })
    }

    pub fn inc(&mut self) -> DifftestsResult<()> {
        match &mut self.state {
            State::None => {
                panic!("test counts not initialized");
            }
            State::Running {
                current_test_count,
                total_test_count,
            } => {
                *current_test_count += 1;
                assert!(*current_test_count <= *total_test_count);
            }
            State::Done | State::Error => {
                panic!("test counts already done");
            }
        }

        self.write_test_counts()?;

        Ok(())
    }

    pub fn test_count_done(&mut self) -> DifftestsResult {
        match self.state {
            State::Done => {}
            State::Running { .. } => {
                self.state = State::Done;
                self.write_test_counts()?;
            }
            _ => panic!("test counts not initialized"),
        }

        Ok(())
    }

    pub fn fail_if_running(&mut self) -> DifftestsResult {
        match self.state {
            State::Running { .. } => {
                self.state = State::Error;
                self.write_test_counts()?;
            }
            _ => {}
        }

        Ok(())
    }

    fn write_test_counts(&self) -> DifftestsResult {
        println!(
            "cargo-difftests-test-counts::{}",
            serde_json::to_string(&self.state)?
        );
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TestRerunnerInvocation {
    tests: Vec<TestInfo>,
}

impl TestRerunnerInvocation {
    pub fn create_invocation_from<'a>(
        iter: impl IntoIterator<Item = &'a AnalyzeAllSingleTest>,
    ) -> DifftestsResult<Self> {
        let mut tests = vec![];

        for g in iter {
            if let Some(difftest) = &g.difftest {
                tests.push(difftest.test_info()?);
            } else {
                // Most likely came from an index.
                tests.push(g.test_info.clone());
            }
        }

        Ok(Self { tests })
    }

    pub fn is_empty(&self) -> bool {
        self.tests.is_empty()
    }

    pub fn tests(&self) -> &[TestInfo] {
        &self.tests
    }

    pub fn test_counts(&self) -> TestRunnerInvocationTestCounts {
        TestRunnerInvocationTestCounts {
            state: State::None,
            _pd: PhantomData,
        }
    }
}

pub const CARGO_DIFFTESTS_VER_NAME: &str = "CARGO_DIFFTESTS_VER";

pub fn read_invocation_from_command_line() -> DifftestsResult<TestRerunnerInvocation> {
    let v = std::env::var(CARGO_DIFFTESTS_VER_NAME).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("missing env var: {}", e),
        )
    })?;

    if v != env!("CARGO_PKG_VERSION") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "version mismatch: expected {} (our version), got {} (cargo-difftests version)",
                env!("CARGO_PKG_VERSION"),
                v
            ),
        )
        .into());
    }

    let mut args = std::env::args().skip(1);

    let f = args.next().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing invocation file")
    })?;

    let invocation_str = std::fs::read_to_string(f)?;
    let invocation = serde_json::from_str(&invocation_str)?;

    Ok(invocation)
}

#[macro_export]
macro_rules! cargo_difftests_test_rerunner {
    ($impl_fn:path) => {
        fn main() -> $crate::DifftestsResult<impl std::process::Termination> {
            let invocation = $crate::test_rerunner_core::read_invocation_from_command_line()?;

            let result = $impl_fn(invocation);

            Ok(result)
        }
    };
}

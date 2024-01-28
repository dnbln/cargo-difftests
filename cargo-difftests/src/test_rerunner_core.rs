use cargo_difftests_core::{CoreGroupDesc, CoreTestDesc};

use crate::{AnalyzeAllSingleTestGroup, DifftestsResult};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TestRerunnerInvocation {
    tests: Vec<CoreTestDesc>,
    groups: Vec<CoreGroupDesc>,
}

impl TestRerunnerInvocation {
    pub fn create_invocation_from<'a>(
        iter: impl IntoIterator<Item = &'a AnalyzeAllSingleTestGroup>,
    ) -> DifftestsResult<Self> {
        let mut tests = vec![];
        let mut groups = vec![];

        for g in iter {
            if let Some(difftest) = &g.difftest {
                tests.push(difftest.load_test_desc()?);
            } else if let Some(difftest_group) = &g.difftest_group {
                groups.push(difftest_group.load_self_json()?);
            } else {
                // Most likely came from an index.
                assert_eq!(g.test_desc.len(), 1);
                let [test] = g.test_desc.as_slice() else { unreachable!() };
                tests.push(test.clone());
            }
        }

        Ok(Self { tests, groups })
    }

    pub fn is_empty(&self) -> bool {
        self.tests.is_empty() && self.groups.is_empty()
    }

    pub fn tests(&self) -> &[CoreTestDesc] {
        &self.tests
    }

    pub fn groups(&self) -> &[CoreGroupDesc] {
        &self.groups
    }
}

pub const CARGO_DIFFTESTS_VER_NAME: &str = "CARGO_DIFFTESTS_VER";

pub fn read_invocation_from_command_line() -> std::io::Result<TestRerunnerInvocation> {
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
        ));
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
        fn main() -> std::io::Result<impl std::process::Termination> {
            let invocation = $crate::test_rerunner_core::read_invocation_from_command_line()?;

            let result = $impl_fn(invocation);

            Ok(result)
        }
    };
}

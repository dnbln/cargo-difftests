use std::{
    borrow::Cow,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use git2::{IntoCString, Oid, Repository};

pub struct CargoProject {
    path: PathBuf,
    test_name: &'static str,
}

pub trait FileContents {
    fn to_content(&self) -> Cow<str>;
}

impl FileContents for str {
    fn to_content(&self) -> Cow<str> {
        Cow::Borrowed(self)
    }
}

impl FileContents for String {
    fn to_content(&self) -> Cow<str> {
        Cow::Borrowed(self)
    }
}

impl FileContents for &str {
    fn to_content(&self) -> Cow<str> {
        Cow::Borrowed(*self)
    }
}

pub struct DifftestsSetupCode<'s>(pub &'s str);

impl<'s> FileContents for DifftestsSetupCode<'s> {
    fn to_content(&self) -> Cow<str> {
        Cow::Owned(format!(
            r#"
#[cfg(cargo_difftests)]
type DifftestsEnv = cargo_difftests_testclient::DifftestsEnv;

#[cfg(not(cargo_difftests))]
type DifftestsEnv = ();

#[cfg(cargo_difftests)]
#[derive(serde::Serialize, Clone)]
struct ExtraArgs {{
    pkg_name: String,
    crate_name: String,
    bin_name: Option<String>,
    test_name: String,
}}

#[must_use]
fn setup_difftests(test_name: &str) -> DifftestsEnv {{
    #[cfg(cargo_difftests)]
    {{
        let tmpdir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("cargo-difftests")
            .join("testsuite")
            .join(env!("DIFFTESTS_TEST_NAME"))
            .join(test_name);
        let difftests_env = cargo_difftests_testclient::init(
            cargo_difftests_testclient::TestDesc::<ExtraArgs> {{
                bin_path: std::env::current_exe().unwrap(),
                extra: ExtraArgs {{
                    pkg_name: env!("CARGO_PKG_NAME").to_string(),
                    crate_name: env!("CARGO_CRATE_NAME").to_string(),
                    bin_name: option_env!("CARGO_BIN_NAME").map(ToString::to_string),
                    test_name: test_name.to_string(),
                }},
            }},
            &tmpdir,
        )
        .unwrap();
        difftests_env
    }}
}}

    {}
        "#,
            self.0
        ))
    }
}

impl CargoProject {
    pub fn edit(&self, file: impl AsRef<Path>, contents: impl FileContents) -> R {
        let p = self.path.join(file);

        if let Some(parent) = p.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        Ok(std::fs::write(p, contents.to_content().as_ref())?)
    }

    fn _internal_run_cargo(&self, args: &[&str]) -> R {
        let output = std::process::Command::new(env!("CARGO"))
            .args(args)
            .current_dir(&self.path)
            .env("DIFFTESTS_TEST_NAME", self.test_name)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;
        report_output_fail("cargo", &output)?;
        Ok(())
    }

    const TARGET_DIR_WS: &'static str = "../../..";

    pub fn run_cargo_build_difftests(&self) -> R {
        self._internal_run_cargo(&["build", "--profile", "difftests"])
    }

    pub fn run_test_difftests(&self, test_name: &str) -> R {
        self._internal_run_cargo(&["test", "--profile", "difftests", test_name, "--", "--exact"])
    }

    pub fn run_all_tests_difftests(&self) -> R {
        self._internal_run_cargo(&["test", "--profile", "difftests"])
    }

    pub fn _internal_cargo_difftests_cmd(&self) -> R<std::process::Command> {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_cargo-difftests"));
        command
            .arg("difftests")
            .current_dir(&self.path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        Ok(command)
    }

    pub fn cargo_difftests(&self) -> R<CargoDifftestsInvocation> {
        let command = self._internal_cargo_difftests_cmd()?;
        Ok(CargoDifftestsInvocation {
            command,
            stdout_match: OutputMatch::None,
            stderr_match: OutputMatch::None,
        })
    }

    pub fn analyze_test(
        &self,
        test_name: &str,
        strategy_info: &TestAnalysisStrategyInfo,
    ) -> R<CargoDifftestsTestAnalysis> {
        let mut command = self._internal_cargo_difftests_cmd()?;
        command.arg("analyze").arg("--dir").arg(format!(
            "{target_dir}/tmp/cargo-difftests/testsuite/{self_test_name}/{test_name}",
            target_dir = Self::TARGET_DIR_WS,
            self_test_name = self.test_name,
        ));
        strategy_info.args_to_cmd(&mut command);
        Ok(CargoDifftestsTestAnalysis { cmd: command })
    }

    pub fn touch_file(&self, path: impl AsRef<Path>) -> R {
        let p = self.path.join(path);
        let d = std::fs::read(&p)?;
        std::fs::write(p, d)?;
        Ok(())
    }

    pub fn load_git_repo(&self) -> R<Repository> {
        Ok(Repository::open(&self.path)?)
    }

    pub fn commit<T: IntoCString>(
        &self,
        repo: &Repository,
        commit_msg: &str,
        path_spec: impl Iterator<Item = T>,
    ) -> R<Oid> {
        let mut index = repo.index()?;
        index.add_all(path_spec, git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let signature =
            git2::Signature::new("John Doe", "johndoe@example.com", &git2::Time::new(0, 0))?;
        let parent_commit = repo.head()?.peel_to_commit()?;
        Ok(repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            commit_msg,
            &repo.find_tree(tree_id)?,
            &[&parent_commit],
        )?)
    }
}

enum OutputMatch {
    None,
    Exact(String),
    Contains(String),
}

impl OutputMatch {
    fn check(&self, s: &str) -> R {
        match self {
            OutputMatch::None => Ok(()),
            OutputMatch::Exact(expected) => {
                if s == expected {
                    Ok(())
                } else {
                    anyhow::bail!("expected: {:?}, got: {:?}", expected, s)
                }
            }
            OutputMatch::Contains(expected) => {
                if s.contains(expected) {
                    Ok(())
                } else {
                    anyhow::bail!("expected to contain: {:?}, got: {:?}", expected, s)
                }
            }
        }
    }
}

pub struct CargoDifftestsInvocation {
    command: std::process::Command,
    stdout_match: OutputMatch,
    stderr_match: OutputMatch,
}

impl CargoDifftestsInvocation {
    pub fn stdout_exact(mut self, expected: &str) -> Self {
        self.stdout_match = OutputMatch::Exact(expected.to_owned());
        self
    }

    pub fn stderr_exact(mut self, expected: &str) -> Self {
        self.stderr_match = OutputMatch::Exact(expected.to_owned());
        self
    }

    pub fn stdout_contains(mut self, expected: &str) -> Self {
        self.stdout_match = OutputMatch::Contains(expected.to_owned());
        self
    }

    pub fn stderr_contains(mut self, expected: &str) -> Self {
        self.stderr_match = OutputMatch::Contains(expected.to_owned());
        self
    }

    pub fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.command.arg(arg);
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command.args(args);
        self
    }

    pub fn run(mut self) -> R {
        let child = self.command.spawn()?;
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8(output.stdout)?;
        let stderr = String::from_utf8(output.stderr)?;
        self.stdout_match.check(&stdout)?;
        self.stderr_match.check(&stderr)?;
        Ok(())
    }
}

#[must_use]
pub struct CargoDifftestsTestAnalysis {
    cmd: std::process::Command,
}

fn report_output_fail(cmd_name: &str, output: &std::process::Output) -> R {
    if !output.status.success() {
        println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        anyhow::bail!("command {cmd_name} failed with status: {}", output.status);
    }

    Ok(())
}

impl CargoDifftestsTestAnalysis {
    fn output_check_and_stdout_check(mut self, expected: &[u8]) -> R<bool> {
        let output = self.cmd.output()?;
        report_output_fail("cargo-difftests", &output)?;
        Ok(output.stdout == expected)
    }

    pub fn is_dirty(self) -> R<bool> {
        self.output_check_and_stdout_check(b"dirty\n")
    }

    pub fn is_clean(self) -> R<bool> {
        self.output_check_and_stdout_check(b"clean\n")
    }

    #[track_caller]
    pub fn assert_is_dirty(self) -> R {
        assert!(self.is_dirty()?);
        Ok(())
    }

    #[track_caller]
    pub fn assert_is_clean(self) -> R {
        assert!(self.is_clean()?);
        Ok(())
    }
}

pub type R<T = ()> = anyhow::Result<T>;

#[derive(Default)]
pub struct CargoProjectConfig {
    pub init_git: bool,
}

pub fn create_cargo_project(
    test_name: &'static str,
    config: CargoProjectConfig,
) -> R<CargoProject> {
    assert!(test_name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_'));

    let wdir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("testsuite");
    let path = wdir.join(test_name);
    if path.exists() {
        std::fs::remove_dir_all(&path)?;
    }

    std::fs::create_dir_all(&path)?;

    let rc_wrapper_difftests = env!("CARGO_BIN_EXE_rustc-wrapper-difftests");
    let rc_wrapper_difftests_workspace = env!("CARGO_BIN_EXE_rustc-wrapper-difftests-workspace");
    std::fs::create_dir(path.join(".cargo"))?;
    std::fs::write(
        path.join(".cargo/config.toml"),
        format!(
            r#"
[profile.difftests]
inherits = "dev"
rustflags = [
  "--cfg", "cargo_difftests",
]

[build]
rustc-wrapper = {rc_wrapper_difftests:?}
rustc-workspace-wrapper = {rc_wrapper_difftests_workspace:?}
target-dir = "{target_dir}"

[env]
RUST_TEST_THREADS = "1"

[unstable]
profile-rustflags = true
    "#,
            target_dir = CargoProject::TARGET_DIR_WS,
        ),
    )?;

    let mut cargo_toml = format!(
        r#"
[package]
name = "{test_name}"
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
cargo-difftests = {{ path = "../../../../cargo-difftests" }}
cargo-difftests-core = {{ path = "../../../../cargo-difftests-core" }}
cargo-difftests-testclient = {{ path = "../../../../cargo-difftests-testclient" }}

anyhow = "1.0.66"
chrono = {{ version = "0.4.23", features = ["serde"] }}
clap = {{ version = "4.0.26", features = ["derive", "string"] }}
git2 = "0.18"
home = "0.5.4"
indoc = "2"
libc = "0.2"
libgit2-sys = "0.16.1"
log = "0.4.17"
path-absolutize = "3.0.14"
path-slash = "0.2.1"
pretty_env_logger = "0.5.0"
proc-macro2 = "1.0.47"
prodash = {{ version = "28.0.0", default-features = false, features = [
    "render-line",
    "render-line-crossterm",
    "render-line-autoconfigure",
    "progress-tree",
] }}
quote = "1.0.21"
rustc-demangle = "0.1.21"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
tempfile = "3.0"
thiserror = "1.0"
    "#
    );

    cargo_toml.push_str(
        r#"
[dependencies.serde]
workspace = true

[dependencies.cargo-difftests-testclient]
workspace = true
"#,
    );

    std::fs::write(path.join("Cargo.toml"), cargo_toml)?;

    if config.init_git {
        let repo = git2::Repository::init(&path)?;
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let signature =
            git2::Signature::new("John Doe", "johndoe@example.com", &git2::Time::new(0, 0))?;
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &repo.find_tree(tree_id)?,
            &[],
        )?;
    }

    Ok(CargoProject { path, test_name })
}

pub fn init_sample_project(test_name: &'static str) -> R<CargoProject> {
    let project = create_cargo_project(test_name, CargoProjectConfig::default())?;
    project.edit(
        "src/lib.rs",
        r#"
        pub fn add(a: i32, b: i32) -> i32 {
            a + b
        }

        pub fn sub(a: i32, b: i32) -> i32 {
            a - b
        }

        mod advanced_arithmetic;
        pub use advanced_arithmetic::*;
    "#,
    )?;
    project.edit(
        "src/advanced_arithmetic.rs",
        r#"
        pub fn mul(a: i32, b: i32) -> i32 {
            a * b
        }

        pub fn div(a: i32, b: i32) -> i32 {
            a / b
        }
    "#,
    )?;

    project.edit(
        "tests/tests.rs",
        DifftestsSetupCode(&format!(
            r#"
        use {test_name}::*;

        #[test]
        fn test_add() {{
            let _env = setup_difftests("test_add");
            assert_eq!(add(2, 2), 4);
        }}

        #[test]
        fn test_sub() {{
            let _env = setup_difftests("test_sub");
            assert_eq!(sub(2, 2), 0);
        }}

        #[test]
        fn test_mul() {{
            let _env = setup_difftests("test_mul");
            assert_eq!(mul(2, 2), 4);
        }}

        #[test]
        fn test_div() {{
            let _env = setup_difftests("test_div");
            assert_eq!(div(2, 2), 1);
        }}
    "#,
        )),
    )?;

    Ok(project)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalysisAlgo {
    #[default]
    FsMtime,
    GitDiffHunks {
        commit: Option<Oid>,
    },
    GitDiffFiles {
        commit: Option<Oid>,
    },
}

impl AnalysisAlgo {
    pub fn git_diff_files_with_head() -> Self {
        Self::GitDiffFiles { commit: None }
    }

    pub fn git_diff_files_with_commit(commit: Oid) -> Self {
        Self::GitDiffFiles {
            commit: Some(commit),
        }
    }

    pub fn git_diff_hunks_with_head() -> Self {
        Self::GitDiffHunks { commit: None }
    }

    pub fn git_diff_hunks_with_commit(commit: Oid) -> Self {
        Self::GitDiffHunks {
            commit: Some(commit),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TestAnalysisStrategyInfo {
    pub algo: AnalysisAlgo,
}

impl TestAnalysisStrategyInfo {
    pub fn args_to_cmd(&self, cmd: &mut std::process::Command) {
        match self.algo {
            AnalysisAlgo::FsMtime => {
                cmd.arg("--algo=fs-mtime");
            }
            AnalysisAlgo::GitDiffHunks { commit } => {
                cmd.arg("--algo=git-diff-hunks");
                if let Some(commit) = commit {
                    cmd.arg("--commit").arg(commit.to_string());
                }
            }
            AnalysisAlgo::GitDiffFiles { commit } => {
                cmd.arg("--algo=git-diff-files");
                if let Some(commit) = commit {
                    cmd.arg("--commit").arg(commit.to_string());
                }
            }
        }
    }
}

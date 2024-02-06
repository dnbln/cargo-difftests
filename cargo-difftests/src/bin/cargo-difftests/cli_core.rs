use std::{
    ffi::OsString,
    fmt::{self, Display, Formatter},
    io::{BufRead, Write},
    path::PathBuf,
};

use anyhow::bail;
use cargo_difftests::{
    analysis::GitDiffStrategy,
    difftest::{DiscoverIndexPathResolver, ExportProfdataConfig},
    AnalysisVerdict, AnalyzeAllSingleTestGroup, IndexCompareDifferences, TouchSameFilesDifference,
};
use clap::{Args, ValueEnum};
use log::info;
use prodash::unit;

use crate::{CargoDifftestsContext, CargoDifftestsResult};
use cargo_difftests::test_rerunner_core::State as TestRunnerState;

#[derive(ValueEnum, Debug, Copy, Clone)]
pub enum FlattenFilesTarget {
    /// Flatten all files to the root of the repository.
    ///
    /// Files outside of the repository will be kept as-is.
    #[clap(name = "repo-root")]
    RepoRoot,
}

impl Display for FlattenFilesTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FlattenFilesTarget::RepoRoot => write!(f, "repo-root"),
        }
    }
}

#[derive(Args, Debug, Copy, Clone)]
pub struct CompileTestIndexFlags {
    /// Whether to flatten all files to a directory.
    #[clap(long)]
    pub flatten_files_to: Option<FlattenFilesTarget>,
    /// Whether to remove the binary path from the difftest info
    /// in the index.
    ///
    /// This is enabled by default, as it is expected to be an absolute
    /// path.
    #[clap(
        long = "no-remove-bin-path",
        default_value_t = true,
        action = clap::ArgAction::SetFalse,
    )]
    pub remove_bin_path: bool,
    /// Whether to generate a full index, or a tiny index.
    ///
    /// The difference lies in the fact that the full index will contain
    /// all the information about the files that were touched by the test,
    /// including line and branch coverage, while the tiny index will only
    /// contain the list of files that were touched by the test.
    ///
    /// The tiny index is much faster to generate, and also much faster to
    /// analyze with, but it does not contain any coverage information that
    /// could be used by the `--algo=git-diff-hunks` algorithm, and as such,
    /// using the `git-diff-hunks` algorithm with an index generated without
    /// the `--full-index` flag will result in an error.
    #[clap(long = "full-index")]
    pub full_index: bool,
    /// Windows-only: Whether to replace all backslashes in paths with
    /// normal forward slashes.
    #[cfg(windows)]
    #[clap(
        long = "no-path-slash-replace",
        default_value_t = true,
        action = clap::ArgAction::SetFalse,
    )]
    pub path_slash_replace: bool,
}

impl Default for CompileTestIndexFlags {
    fn default() -> Self {
        Self {
            flatten_files_to: Some(FlattenFilesTarget::RepoRoot),
            remove_bin_path: true,
            full_index: false,
            #[cfg(windows)]
            path_slash_replace: true,
        }
    }
}

#[derive(ValueEnum, Debug, Copy, Clone, Default)]
pub enum AnalysisIndexStrategy {
    /// Will always use indexes.
    ///
    /// If the indexes are not available, or they are outdated,
    /// they will be re-generated, and then the analysis will use
    /// the indexes.
    #[clap(name = "always")]
    Always,
    /// Will use indexes if they are available,
    /// but if they are not available, it will not generate them,
    /// and instead use a slightly slower algorithm to work with data
    /// straight from `llvm-cov export` instead.
    #[clap(name = "if-available")]
    IfAvailable,
    /// Will never use indexes.
    #[default]
    #[clap(name = "never")]
    Never,
    /// Will always use indexes, and will also clean up the difftest
    /// directory of all the profiling data, which should in theory
    /// not be needed anymore, as the analysis can run on index data alone,
    /// unless using the `never` strategy in subsequent calls of `cargo-difftests`.
    #[clap(name = "always-and-clean")]
    AlwaysAndClean,
}

impl Display for AnalysisIndexStrategy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisIndexStrategy::Always => write!(f, "always"),
            AnalysisIndexStrategy::IfAvailable => write!(f, "if-available"),
            AnalysisIndexStrategy::Never => write!(f, "never"),
            AnalysisIndexStrategy::AlwaysAndClean => write!(f, "always-and-clean"),
        }
    }
}

#[derive(Args, Debug)]
pub struct DifftestDir {
    /// The path to the difftest directory.
    ///
    /// This should be the directory that was passed
    /// to `cargo_difftests_testclient::init`.
    #[clap(long)]
    pub dir: PathBuf,
}

#[derive(Debug, Clone)]
pub enum IndexPathOrResolve {
    Resolve,
    Path(PathBuf),
}

impl From<OsString> for IndexPathOrResolve {
    fn from(s: OsString) -> Self {
        if s == "resolve" {
            IndexPathOrResolve::Resolve
        } else {
            IndexPathOrResolve::Path(PathBuf::from(s))
        }
    }
}

#[derive(ValueEnum, Debug, Copy, Clone, Default)]
pub enum IndexesTouchSameFilesReportAction {
    /// Print the report to stdout.
    #[default]
    #[clap(name = "print")]
    Print,
    /// Assert that the indexes touch the same files.
    ///
    /// If they do not, the program will exit with a non-zero exit code.
    #[clap(name = "assert")]
    Assert,
}

impl IndexesTouchSameFilesReportAction {
    pub fn do_for_report(
        &self,
        report: Result<(), IndexCompareDifferences<TouchSameFilesDifference>>,
    ) -> CargoDifftestsResult {
        match self {
            IndexesTouchSameFilesReportAction::Print => match report {
                Ok(()) => {
                    println!("[]");

                    Ok(())
                }
                Err(diffs) => {
                    let s = serde_json::to_string(diffs.differences())?;

                    println!("{s}");

                    Ok(())
                }
            },
            IndexesTouchSameFilesReportAction::Assert => match report {
                Ok(()) => Ok(()),
                Err(e) => {
                    let s = serde_json::to_string(e.differences())?;

                    eprintln!("{s}");

                    bail!("indexes do not touch the same files")
                }
            },
        }
    }
}

impl Display for IndexesTouchSameFilesReportAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexesTouchSameFilesReportAction::Print => write!(f, "print"),
            IndexesTouchSameFilesReportAction::Assert => write!(f, "assert"),
        }
    }
}

/// The algorithm to use for the analysis.
#[derive(ValueEnum, Debug, Copy, Clone, Default)]
pub enum DirtyAlgorithm {
    /// Use file system mtimes to find the files that have changed.
    ///
    /// This is the fastest algorithm, but it is not very accurate.
    #[default]
    #[clap(name = "fs-mtime")]
    FsMtime,
    /// Use the list of files from `git diff`.
    ///
    /// This is a bit slower than `fs-mtime`.
    ///
    /// Warning: not very accurate if not used well.
    /// See the introductory blog post for more details.
    #[clap(name = "git-diff-files")]
    GitDiffFiles,
    /// Use the list of diff hunks from `git diff` to compute the changed files.
    ///
    /// This is a bit slower than `fs-mtime`.
    ///
    /// Warning: like `git-diff-files`, it is not very accurate if not used well.
    /// See the introductory blog post for more details.
    #[clap(name = "git-diff-hunks")]
    GitDiffHunks,
}

impl DirtyAlgorithm {
    pub fn convert(self, commit: Option<git2::Oid>) -> cargo_difftests::analysis::DirtyAlgorithm {
        match self {
            DirtyAlgorithm::FsMtime => cargo_difftests::analysis::DirtyAlgorithm::FileSystemMtimes,
            DirtyAlgorithm::GitDiffFiles => cargo_difftests::analysis::DirtyAlgorithm::GitDiff {
                strategy: GitDiffStrategy::FilesOnly,
                commit,
            },
            DirtyAlgorithm::GitDiffHunks => cargo_difftests::analysis::DirtyAlgorithm::GitDiff {
                strategy: GitDiffStrategy::Hunks,
                commit,
            },
        }
    }
}

impl Display for DirtyAlgorithm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DirtyAlgorithm::FsMtime => write!(f, "fs-mtime"),
            DirtyAlgorithm::GitDiffFiles => write!(f, "git-diff-files"),
            DirtyAlgorithm::GitDiffHunks => write!(f, "git-diff-hunks"),
        }
    }
}

#[derive(Args, Debug)]
pub struct AnalysisIndex {
    /// The root directory where all index files will be stored.
    ///
    /// Only used if `--index-strategy` is set to `always`, `always-and-clean`
    /// or `if-available`, otherwise ignored.
    #[clap(
        long,
        required_if_eq_any = [
            ("index_strategy", "always"),
            ("index_strategy", "always-and-clean"),
            ("index_strategy", "if-available"),
        ]
    )]
    pub index_root: Option<PathBuf>,
    /// The strategy to use for the analysis index.
    #[clap(long, default_value_t = Default::default())]
    pub index_strategy: AnalysisIndexStrategy,
    #[clap(flatten)]
    pub compile_test_index_flags: CompileTestIndexFlags,
}

#[derive(thiserror::Error, Debug)]
pub enum IndexResolverError {
    #[error("--root was not provided, but was required by the --index-strategy")]
    RootIsNone,
}

impl AnalysisIndex {
    pub fn index_resolver(
        &self,
        root: Option<PathBuf>,
    ) -> Result<Option<DiscoverIndexPathResolver>, IndexResolverError> {
        match self.index_strategy {
            AnalysisIndexStrategy::Always | AnalysisIndexStrategy::AlwaysAndClean => {
                let index_root = self.index_root.as_ref().unwrap(); // should be set by clap

                Ok(Some(DiscoverIndexPathResolver::Remap {
                    from: root.ok_or(IndexResolverError::RootIsNone)?,
                    to: index_root.clone(),
                }))
            }
            AnalysisIndexStrategy::IfAvailable => {
                let index_root = self.index_root.as_ref().unwrap(); // should be set by clap

                Ok(Some(DiscoverIndexPathResolver::Remap {
                    from: root.ok_or(IndexResolverError::RootIsNone)?,
                    to: index_root.clone(),
                }))
            }
            AnalysisIndexStrategy::Never => Ok(None),
        }
    }
}

#[derive(Args, Debug)]
pub struct AlgoArgs {
    /// The algorithm to use to find the "dirty" files.
    #[clap(long, default_value_t = Default::default())]
    pub algo: DirtyAlgorithm,
    /// Optionally, if the algorithm is `git-diff-files` or `git-diff-hunks`,
    /// through this option we can specify another commit to use as the base
    /// for the diff.
    ///
    /// By default, the commit `HEAD` points to will be used.
    #[clap(long)]
    pub commit: Option<git2::Oid>,
}

#[derive(Args, Debug, Clone)]
pub struct OtherBinaries {
    /// Any other binaries to use for the analysis.
    ///
    /// By default, the only binary that `cargo-difftests` uses will
    /// be the `bin_path` from the test description (passed to
    /// `cargo_difftests_testclient::init`), but if the test spawned other
    /// children subprocesses that were profiled, and should be used in the
    /// analysis, then the paths to those binaries should be passed here.
    #[clap(long = "bin")]
    pub other_binaries: Vec<PathBuf>,
}

#[derive(Args, Debug, Clone, Copy)]
pub struct IgnoreRegistryFilesFlag {
    /// Whether to ignore files from the cargo registry.
    ///
    /// This is enabled by default, as files in the cargo registry are not
    /// expected to be modified by the user.
    ///
    /// If you want to include files from the cargo registry, use the
    /// `--no-ignore-registry-files` flag.
    #[clap(
        long = "no-ignore-registry-files",
        default_value_t = true,
        action = clap::ArgAction::SetFalse,
    )]
    pub ignore_registry_files: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ExportProfdataConfigFlags {
    #[clap(flatten)]
    other_binaries: OtherBinaries,
}

impl ExportProfdataConfigFlags {
    pub fn config(&self, ignore_registry_files: IgnoreRegistryFilesFlag) -> ExportProfdataConfig {
        ExportProfdataConfig {
            ignore_registry_files: ignore_registry_files.ignore_registry_files,
            other_binaries: self.other_binaries.other_binaries.clone(),
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, Default)]
pub enum AnalyzeAllActionKind {
    /// Print the report to stdout.
    #[default]
    #[clap(name = "print")]
    Print,
    /// Assert that all the tests are clean.
    ///
    /// If any of them is dirty, the program will exit with a non-zero exit code.
    #[clap(name = "assert-clean")]
    AssertClean,
    /// Rerun all the dirty tests.
    #[clap(name = "rerun-dirty")]
    RerunDirty,
}

impl fmt::Display for AnalyzeAllActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyzeAllActionKind::Print => write!(f, "print"),
            AnalyzeAllActionKind::AssertClean => write!(f, "assert-clean"),
            AnalyzeAllActionKind::RerunDirty => write!(f, "rerun-dirty"),
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct RerunRunner {
    /// The runner to use for the `rerun-dirty` action.
    #[clap(long, default_value = "cargo-difftests-default-rerunner")]
    pub runner: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct AnalyzeAllActionArgs {
    /// The action to take for the report.
    #[clap(long, default_value_t = Default::default())]
    pub action: AnalyzeAllActionKind,
    #[clap(flatten)]
    pub runner: RerunRunner,
}

impl AnalyzeAllActionArgs {
    pub fn perform_for(
        &self,
        ctxt: &CargoDifftestsContext,
        results: &[AnalyzeAllSingleTestGroup],
    ) -> CargoDifftestsResult {
        match self.action {
            AnalyzeAllActionKind::Print => {
                let out_json = serde_json::to_string(&results)?;
                println!("{out_json}");
            }
            AnalyzeAllActionKind::AssertClean => {
                let dirty = results.iter().any(|r| r.verdict == AnalysisVerdict::Dirty);

                if dirty {
                    bail!("some tests are dirty")
                }
            }
            AnalyzeAllActionKind::RerunDirty => {
                let invocation =
                    cargo_difftests::test_rerunner_core::TestRerunnerInvocation::create_invocation_from(
                        results
                        .iter()
                        .filter(|r| r.verdict == AnalysisVerdict::Dirty),
                    )?;

                if invocation.is_empty() {
                    return Ok(());
                }

                let mut pb = ctxt.new_child("Rerunning dirty tests");
                pb.init(Some(1), Some(unit::label("test sets")));

                let invocation_str = serde_json::to_string(&invocation)?;

                let mut invocation_file = tempfile::NamedTempFile::new()?;
                write!(&mut invocation_file, "{}", invocation_str)?;
                invocation_file.flush()?;

                let mut cmd = std::process::Command::new(&self.runner.runner);
                cmd.arg(invocation_file.path())
                    .env(
                        cargo_difftests::test_rerunner_core::CARGO_DIFFTESTS_VER_NAME,
                        env!("CARGO_PKG_VERSION"),
                    )
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                let mut child = cmd.spawn()?;

                let mut stdout_child = child.stdout.take().unwrap();
                let mut stderr_child = child.stderr.take().unwrap();

                let tests = pb.add_child("Tests");
                let handle = std::thread::spawn(move || {
                    let mut tests = tests;
                    let mut tests_initialized = false;
                    for line in std::io::BufReader::new(&mut stdout_child).lines() {
                        let line = line?;
                        if line.starts_with("cargo-difftests-test-counts::") {
                            let l = line.trim_start_matches("cargo-difftests-test-counts::");
                            let counts: TestRunnerState = serde_json::from_str(l)?;
                            match counts {
                                TestRunnerState::None => {}
                                TestRunnerState::Running {
                                    current_test_count,
                                    total_test_count,
                                } => {
                                    if !tests_initialized {
                                        tests.init(
                                            Some(total_test_count),
                                            Some(unit::label("tests")),
                                        );
                                        tests_initialized = true;
                                    }

                                    tests.set(current_test_count);
                                }
                                TestRunnerState::Done => {
                                    tests.done("Tests are done");
                                }
                                TestRunnerState::Error => {
                                    tests.fail("Tests failed");
                                }
                            }
                        } else if line.starts_with("cargo-difftests-start-test::") {
                            let t = line.trim_start_matches("cargo-difftests-start-test::");
                            tests.info(format!("Running test {t}"));
                        } else if line.starts_with("cargo-difftests-test-successful::") {
                            let t = line.trim_start_matches("cargo-difftests-test-successful::");
                            tests.info(format!("Test {t} successful"));
                        } else if line.starts_with("cargo-difftests-test-failed::") {
                            let t = line.trim_start_matches("cargo-difftests-test-failed::");
                            tests.info(format!("Test {t} failed"));
                        } else {
                            info!("rerun stdout: {line}");
                        }
                    }

                    Ok::<_, anyhow::Error>(())
                });

                let status = child.wait()?;

                handle.join().unwrap()?;

                for line in std::io::BufReader::new(&mut stderr_child).lines() {
                    let line = line?;
                    info!("rerun stderr: {line}");
                }

                pb.inc();

                match status.exit_ok() {
                    Ok(()) => {
                        pb.done("Rerun successful");
                    }
                    Err(e) => {
                        pb.fail("Rerun failed");
                        bail!(e);
                    }
                }
            }
        }
        Ok(())
    }
}

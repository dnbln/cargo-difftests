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
    AnalysisVerdict, AnalyzeAllSingleTest, IndexCompareDifferences, TouchSameFilesDifference,
};
use clap::{Args, ValueEnum};
use log::{error, info};
use prodash::unit;

use crate::{
    ops::{self, core::cargo_bin_path},
    CargoDifftestsContext, CargoDifftestsResult,
};

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
    #[clap(long)]
    pub compile_index: bool,
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
    #[error("--index-root was not provided, but was required by the --index-strategy")]
    IndexRootIsNone,
    #[error("--root was not provided, but was required by the --index-strategy")]
    RootIsNone,
}

impl AnalysisIndex {
    pub fn index_resolver(
        &self,
        root: Option<PathBuf>,
    ) -> Result<Option<DiscoverIndexPathResolver>, IndexResolverError> {
        let index_s = if self.compile_index {
            AnalysisIndexStrategy::Always
        } else {
            self.index_strategy
        };

        match index_s {
            AnalysisIndexStrategy::Always | AnalysisIndexStrategy::AlwaysAndClean => {
                let index_root = self
                    .index_root
                    .as_ref()
                    .ok_or(IndexResolverError::IndexRootIsNone)?; // should be set by clap

                Ok(Some(DiscoverIndexPathResolver::Remap {
                    from: root.ok_or(IndexResolverError::RootIsNone)?,
                    to: index_root.clone(),
                }))
            }
            AnalysisIndexStrategy::IfAvailable => {
                let index_root = self
                    .index_root
                    .as_ref()
                    .ok_or(IndexResolverError::IndexRootIsNone)?; // should be set by clap

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
    pub other_binaries: OtherBinaries,
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
        results: &[AnalyzeAllSingleTest],
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
                ops::core::rerun_dirty(&ctxt, results, &self.runner)?;
            }
        }
        Ok(())
    }
}

#[derive(Args, Debug, Clone)]
pub struct DifftestsRoot {
    /// The root directory where all the difftests were stored.
    ///
    /// Needs to be known to be able to properly remap the paths
    /// to the index files, and is therefore only required if the
    /// `--index-strategy` is `always`, `always-and-clean`, or
    /// `if-available`.
    #[clap(long, env = "CARGO_DIFFTESTS_ROOT", default_value = get_default_difftests_dir().unwrap())]
    pub root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct DifftestsRootRequired {
    /// The root directory where all the difftests were stored.
    ///
    /// Needs to be known to be able to properly remap the paths
    /// to the index files, and is therefore only required if the
    /// `--index-strategy` is `always`, `always-and-clean`, or
    /// `if-available`.
    #[clap(long, env = "CARGO_DIFFTESTS_ROOT", default_value = get_default_difftests_dir().unwrap())]
    pub root: PathBuf,
}

pub fn get_target_dir() -> CargoDifftestsResult<PathBuf> {
    #[derive(serde::Deserialize)]
    struct Meta {
        target_directory: PathBuf,
    }

    let o = std::process::Command::new(cargo_bin_path())
        .args(&["metadata", "--no-deps", "--format-version", "1"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()?;

    if !o.status.success() {
        let stderr = String::from_utf8(o.stderr)?;
        error!("cargo metadata failed:\n{}", stderr);
        bail!("cargo metadata failed: {}", stderr);
    }

    let meta: Meta = serde_json::from_slice(&o.stdout)?;
    Ok(meta.target_directory)
}

fn get_default_difftests_dir() -> CargoDifftestsResult<OsString> {
    let target_dir = get_target_dir()?;
    Ok(target_dir.join("tmp").join("difftests").into_os_string())
}

#[derive(Args, Debug, Clone)]
pub struct DifftestsRootDir {
    /// The root directory where all the difftests were stored.
    ///
    /// Needs to be known to be able to properly remap the paths
    /// to the index files, and is therefore only required if the
    /// `--index-strategy` is `always`, `always-and-clean`, or
    /// `if-available`.
    ///
    /// (Also for discovery)
    #[clap(long, default_value = "target/tmp/cargo-difftests")]
    pub dir: PathBuf,
}

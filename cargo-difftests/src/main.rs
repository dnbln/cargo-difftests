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

use core::fmt;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use cargo_difftests::analysis::{
    file_is_from_cargo_registry, AnalysisConfig, AnalysisContext, AnalysisResult, GitDiffStrategy,
};
use cargo_difftests::difftest::{Difftest, DiscoverIndexPathResolver, ExportProfdataConfig};
use cargo_difftests::group_difftest::GroupDifftestGroup;
use cargo_difftests::index_data::{IndexDataCompilerConfig, IndexSize, TestIndex};
use cargo_difftests::{
    AnalysisVerdict, AnalyzeAllSingleTestGroup, IndexCompareDifferences, TouchSameFilesDifference,
};
use clap::{Args, Parser, ValueEnum};
use log::warn;

#[derive(Args, Debug)]
pub struct ExportProfdataCommand {
    #[clap(flatten)]
    export_profdata_config_flags: ExportProfdataConfigFlags,
}

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
    #[clap(flatten)]
    ignore_cargo_registry: IgnoreRegistryFilesFlag,
    /// Whether to flatten all files to a directory.
    #[clap(long)]
    flatten_files_to: Option<FlattenFilesTarget>,
    /// Whether to remove the binary path from the difftest info
    /// in the index.
    ///
    /// This is enabled by default, as it is expected to be an absolute
    /// path.
    #[clap(
        long = "no-remove-bin-path",
        default_value_t = true,
        action(clap::ArgAction::SetFalse)
    )]
    remove_bin_path: bool,
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
    full_index: bool,
    /// Windows-only: Whether to replace all backslashes in paths with
    /// normal forward slashes.
    #[cfg(windows)]
    #[clap(
        long = "no-path-slash-replace",
        default_value_t = true,
        action(clap::ArgAction::SetFalse)
    )]
    path_slash_replace: bool,
}

impl Default for CompileTestIndexFlags {
    fn default() -> Self {
        Self {
            ignore_cargo_registry: IgnoreRegistryFilesFlag {
                ignore_registry_files: true,
            },
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

#[derive(Parser, Debug)]
pub enum LowLevelCommand {
    /// Run the `llvm-profdata merge` command, to merge all
    /// the `.profraw` files from a difftest directory into
    /// a single `.profdata` file.
    MergeProfdata {
        #[clap(flatten)]
        dir: DifftestDir,
        /// Whether to force the merge.
        ///
        /// If this flag is not passed, and the `.profdata` file
        /// already exists, the merge will not be run.
        #[clap(long)]
        force: bool,
    },
    /// Run the `llvm-cov export` command, to export the
    /// `.profdata` file into a `.json` file that can be later
    /// used for analysis.
    ExportProfdata {
        #[clap(flatten)]
        dir: DifftestDir,
        #[clap(flatten)]
        cmd: ExportProfdataCommand,
    },
    /// Run the analysis for a single difftest directory.
    RunAnalysis {
        #[clap(flatten)]
        dir: DifftestDir,
        #[clap(flatten)]
        algo: AlgoArgs,
    },
    /// Compile a test index for a single difftest directory.
    CompileTestIndex {
        #[clap(flatten)]
        dir: DifftestDir,
        /// The output file to write the index to.
        #[clap(short, long)]
        output: PathBuf,
        #[clap(flatten)]
        export_profdata_config_flags: ExportProfdataConfigFlags,
        #[clap(flatten)]
        compile_test_index_flags: CompileTestIndexFlags,
    },
    /// Runs the analysis for a single test index.
    RunAnalysisWithTestIndex {
        /// The path to the test index.
        #[clap(long)]
        index: PathBuf,
        #[clap(flatten)]
        algo: AlgoArgs,
    },
    /// Compare two test indexes, by the files that they "touch"
    /// (have regions that have an execution count > 0).
    IndexesTouchSameFilesReport {
        /// The first index to compare.
        index1: PathBuf,
        /// The second index to compare.
        index2: PathBuf,
        /// The action to take for the report.
        #[clap(long, default_value_t = Default::default())]
        action: IndexesTouchSameFilesReportAction,
    },
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
    fn do_for_report(
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
    fn convert(self, commit: Option<git2::Oid>) -> cargo_difftests::analysis::DirtyAlgorithm {
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
    index_root: Option<PathBuf>,
    /// The strategy to use for the analysis index.
    #[clap(long, default_value_t = Default::default())]
    index_strategy: AnalysisIndexStrategy,
    #[clap(flatten)]
    compile_test_index_flags: CompileTestIndexFlags,
}

#[derive(thiserror::Error, Debug)]
pub enum IndexResolverError {
    #[error("--root was not provided, but was required by the --index-strategy")]
    RootIsNone,
}

impl AnalysisIndex {
    fn index_resolver(
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
    algo: DirtyAlgorithm,
    /// Optionally, if the algorithm is `git-diff-files` or `git-diff-hunks`,
    /// through this option we can specify another commit to use as the base
    /// for the diff.
    ///
    /// By default, the commit `HEAD` points to will be used.
    #[clap(long)]
    commit: Option<git2::Oid>,
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
    other_binaries: Vec<PathBuf>,
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
        action(clap::ArgAction::SetFalse)
    )]
    ignore_registry_files: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ExportProfdataConfigFlags {
    #[clap(flatten)]
    ignore_registry_files: IgnoreRegistryFilesFlag,
    #[clap(flatten)]
    other_binaries: OtherBinaries,
}

impl ExportProfdataConfigFlags {
    fn config(&self) -> ExportProfdataConfig {
        ExportProfdataConfig {
            ignore_registry_files: self.ignore_registry_files.ignore_registry_files,
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
    runner: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct AnalyzeAllActionArgs {
    /// The action to take for the report.
    #[clap(long, default_value_t = Default::default())]
    action: AnalyzeAllActionKind,
    #[clap(flatten)]
    runner: RerunRunner,
}

impl AnalyzeAllActionArgs {
    fn perform_for(&self, results: &[AnalyzeAllSingleTestGroup]) -> CargoDifftestsResult {
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
                        .filter(|r| r.verdict == AnalysisVerdict::Dirty)
                    )?;

                if invocation.is_empty() {
                    return Ok(());
                }

                let invocation_str = serde_json::to_string(&invocation)?;

                let mut invocation_file = tempfile::NamedTempFile::new()?;
                write!(&mut invocation_file, "{}", invocation_str)?;
                invocation_file.flush()?;

                let mut cmd = std::process::Command::new(&self.runner.runner);
                cmd.arg(invocation_file.path()).env(
                    cargo_difftests::test_rerunner_core::CARGO_DIFFTESTS_VER_NAME,
                    env!("CARGO_PKG_VERSION"),
                );

                let status = cmd.status()?;

                status.exit_ok()?;
            }
        }
        Ok(())
    }
}

#[derive(Parser, Debug)]
pub enum App {
    /// Discover the difftests from a given directory.
    DiscoverDifftests {
        /// The root directory where all the difftests were stored.
        ///
        /// This should be some common ancestor directory of all
        /// the paths passed to `cargo_difftests_testclient::init`.
        #[clap(long, default_value = "target/tmp/cargo-difftests")]
        dir: PathBuf,
        /// The directory where the index files were stored, if any.
        #[clap(long)]
        index_root: Option<PathBuf>,
        /// With this flag, `cargo-difftests` will ignore any incompatible difftest and continue.
        ///
        /// Without this flag, when `cargo-difftests` finds an
        /// incompatible difftest on-disk, it will fail.
        #[clap(long)]
        ignore_incompatible: bool,
    },
    /// Analyze a single difftest.
    Analyze {
        #[clap(flatten)]
        dir: DifftestDir,
        /// Whether to force the generation of intermediary files.
        ///
        /// Without this flag, if the intermediary files are already present,
        /// they will be used instead of being regenerated.
        #[clap(long)]
        force: bool,
        #[clap(flatten)]
        algo: AlgoArgs,
        #[clap(flatten)]
        export_profdata_config_flags: ExportProfdataConfigFlags,
        #[clap(flatten)]
        analysis_index: AnalysisIndex,
        /// The root directory where all the difftests were stored.
        ///
        /// Needs to be known to be able to properly remap the paths
        /// to the index files, and is therefore only required if the
        /// `--index-strategy` is `always`, `always-and-clean`, or
        /// `if-available`.
        #[clap(long, default_value = "target/tmp/cargo-difftests")]
        root: Option<PathBuf>,
    },
    /// Treats all the difftests found in the given directory as a single
    /// group, and analyzes them together.
    AnalyzeGroup {
        /// The root directory where the difftest group wes stored.
        #[clap(long, default_value = "target/tmp/cargo-difftests")]
        dir: PathBuf,
        /// Whether to force the generation of intermediary files.
        ///
        /// Without this flag, if the intermediary files are already present,
        /// they will be used instead of being regenerated.
        #[clap(long)]
        force: bool,
        #[clap(flatten)]
        algo: AlgoArgs,
        #[clap(flatten)]
        other_binaries: OtherBinaries,
        #[clap(flatten)]
        analysis_index: AnalysisIndex,
        /// The root directory where all the difftests were stored.
        ///
        /// Needs to be known to be able to properly remap the paths
        /// to the index files, and is therefore only required if the
        /// `--index-strategy` is `always`, `always-and-clean`, or
        /// `if-available`.
        #[clap(long, default_value = "target/tmp/cargo-difftests")]
        root: Option<PathBuf>,
    },
    /// Analyze all the difftests in a given directory.
    ///
    /// This is somewhat equivalent to running `cargo difftests discover-difftests`,
    /// and then `cargo difftests analyze` on each of the discovered difftests.
    AnalyzeAll {
        /// The root directory where all the difftests were stored.
        ///
        /// This should be some common ancestor directory of all
        /// the paths passed to `cargo_difftests_testclient::init`.
        #[clap(long, default_value = "target/tmp/cargo-difftests")]
        dir: PathBuf,
        /// Whether to force the generation of intermediary files.
        ///
        /// Without this flag, if the intermediary files are already present,
        /// they will be used instead of being regenerated.
        #[clap(long)]
        force: bool,
        #[clap(flatten)]
        algo: AlgoArgs,
        #[clap(flatten)]
        export_profdata_config_flags: ExportProfdataConfigFlags,
        #[clap(flatten)]
        analysis_index: AnalysisIndex,
        /// With this flag, `cargo-difftests` will ignore any incompatible
        /// difftest and continue.
        ///
        /// Without this flag, when `cargo-difftests` finds an
        /// incompatible difftest on-disk, it will fail.
        #[clap(long)]
        ignore_incompatible: bool,
        #[clap(flatten)]
        action_args: AnalyzeAllActionArgs,
    },
    /// Analyze all the difftests in a given directory, using their index files.
    ///
    /// Note that this does not require the outputs of the difftests to be
    /// present on-disk, and can be used to analyze difftests that were
    /// run on a different machine (given correct flags when
    /// compiling the index).
    AnalyzeAllFromIndex {
        /// The root directory where all the index files are stored.
        #[clap(long)]
        index_root: PathBuf,
        #[clap(flatten)]
        algo: AlgoArgs,
        #[clap(flatten)]
        action_args: AnalyzeAllActionArgs,
    },
    LowLevel {
        #[clap(subcommand)]
        cmd: LowLevelCommand,
    },
}

#[derive(Parser, Debug)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
pub enum CargoApp {
    Difftests {
        #[clap(subcommand)]
        app: App,
    },
}

pub type CargoDifftestsResult<T = ()> = anyhow::Result<T>;

fn resolver_for_index_root(
    tmpdir_root: &Path,
    index_root: Option<PathBuf>,
) -> Option<DiscoverIndexPathResolver> {
    index_root.map(|index_root| DiscoverIndexPathResolver::Remap {
        from: tmpdir_root.to_path_buf(),
        to: index_root,
    })
}

fn discover_difftests(
    dir: PathBuf,
    index_root: Option<PathBuf>,
    ignore_incompatible: bool,
) -> CargoDifftestsResult<Vec<Difftest>> {
    if !dir.exists() || !dir.is_dir() {
        warn!("Directory {} does not exist", dir.display());
        return Ok(vec![]);
    }

    let resolver = resolver_for_index_root(&dir, index_root);

    let discovered = cargo_difftests::difftest::discover_difftests(
        &dir,
        ignore_incompatible,
        resolver.as_ref(),
    )?;

    Ok(discovered)
}

fn run_discover_difftests(
    dir: PathBuf,
    index_root: Option<PathBuf>,
    ignore_incompatible: bool,
) -> CargoDifftestsResult {
    let discovered = discover_difftests(dir, index_root, ignore_incompatible)?;
    let s = serde_json::to_string(&discovered)?;
    println!("{s}");

    Ok(())
}

fn run_merge_profdata(dir: PathBuf, force: bool) -> CargoDifftestsResult {
    // we do not need the index resolver here, because we are not going to use the index
    let mut discovered = Difftest::discover_from(dir, None)?;

    discovered.merge_profraw_files_into_profdata(force)?;

    Ok(())
}

fn run_export_profdata(dir: PathBuf, cmd: ExportProfdataCommand) -> CargoDifftestsResult {
    // we do not need the index resolver here, because we are not going to use the index
    let discovered = Difftest::discover_from(dir, None)?;

    if !discovered.has_profdata() {
        bail!("difftest directory does not have a .profdata file");
    }

    let coverage = discovered.export_profdata(cmd.export_profdata_config_flags.config())?;

    let s = serde_json::to_string(&coverage)?;

    println!("{s}");

    Ok(())
}

fn display_analysis_result(r: AnalysisResult) {
    let res = match r {
        AnalysisResult::Clean => "clean",
        AnalysisResult::Dirty => "dirty",
    };

    println!("{res}");
}

fn run_analysis(
    dir: PathBuf,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
) -> CargoDifftestsResult {
    let mut discovered = Difftest::discover_from(dir, None)?;

    assert!(discovered.has_profdata());

    let mut analysis_cx = discovered.start_analysis(ExportProfdataConfig {
        ignore_registry_files: true,
        other_binaries: vec![],
    })?;

    analysis_cx.run(&AnalysisConfig {
        dirty_algorithm: algo.convert(commit),
        error_on_invalid_config: true,
    })?;

    let r = analysis_cx.finish_analysis();

    display_analysis_result(r);

    Ok(())
}

fn run_analysis_with_test_index(
    index: PathBuf,
    dirty_algorithm: DirtyAlgorithm,
    commit: Option<git2::Oid>,
) -> CargoDifftestsResult {
    let mut analysis_cx = AnalysisContext::with_index_from(&index)?;

    analysis_cx.run(&AnalysisConfig {
        dirty_algorithm: dirty_algorithm.convert(commit),
        error_on_invalid_config: true,
    })?;

    let r = analysis_cx.finish_analysis();

    display_analysis_result(r);

    Ok(())
}

fn compile_test_index_config(
    compile_test_index_flags: CompileTestIndexFlags,
) -> CargoDifftestsResult<IndexDataCompilerConfig> {
    let flatten_root = match compile_test_index_flags.flatten_files_to {
        Some(FlattenFilesTarget::RepoRoot) => {
            let repo = git2::Repository::open_from_env()?;
            let root = repo.workdir().context("repo has no workdir")?;
            Some(root.to_path_buf())
        }
        None => None,
    };

    let config = IndexDataCompilerConfig {
        ignore_registry_files: true,
        index_filename_converter: Box::new(move |path| {
            let p = match &flatten_root {
                Some(root) => path.strip_prefix(root).unwrap_or(path),
                None => path,
            };

            #[cfg(windows)]
            let p = if compile_test_index_flags.path_slash_replace {
                use path_slash::PathExt;

                PathBuf::from(p.to_slash().unwrap().into_owned())
            } else {
                p.to_path_buf()
            };

            #[cfg(not(windows))]
            let p = p.to_path_buf();

            p
        }),
        accept_file: Box::new(move |path| {
            if compile_test_index_flags
                .ignore_cargo_registry
                .ignore_registry_files
                && file_is_from_cargo_registry(path)
            {
                return false;
            }

            true
        }),
        remove_bin_path: compile_test_index_flags.remove_bin_path,
        index_size: if compile_test_index_flags.full_index {
            IndexSize::Full
        } else {
            IndexSize::Tiny
        },
    };

    Ok(config)
}

fn run_compile_test_index(
    dir: PathBuf,
    output: PathBuf,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    compile_test_index_flags: CompileTestIndexFlags,
) -> CargoDifftestsResult {
    let discovered = Difftest::discover_from(dir, None)?;
    assert!(discovered.has_profdata());

    let config = compile_test_index_config(compile_test_index_flags)?;

    let result =
        discovered.compile_test_index_data(export_profdata_config_flags.config(), config)?;

    result.write_to_file(&output)?;

    Ok(())
}

fn run_indexes_touch_same_files_report(
    index1: PathBuf,
    index2: PathBuf,
    action: IndexesTouchSameFilesReportAction,
) -> CargoDifftestsResult {
    let index1 = TestIndex::read_from_file(&index1)?;
    let index2 = TestIndex::read_from_file(&index2)?;

    let report = cargo_difftests::compare_indexes_touch_same_files(&index1, &index2);

    action.do_for_report(report)?;

    Ok(())
}

fn run_low_level_cmd(cmd: LowLevelCommand) -> CargoDifftestsResult {
    match cmd {
        LowLevelCommand::MergeProfdata { dir, force } => {
            run_merge_profdata(dir.dir, force)?;
        }
        LowLevelCommand::ExportProfdata { dir, cmd } => {
            run_export_profdata(dir.dir, cmd)?;
        }
        LowLevelCommand::RunAnalysis {
            dir,
            algo: AlgoArgs { algo, commit },
        } => {
            run_analysis(dir.dir, algo, commit)?;
        }
        LowLevelCommand::CompileTestIndex {
            dir,
            output,
            export_profdata_config_flags,
            compile_test_index_flags,
        } => {
            run_compile_test_index(
                dir.dir,
                output,
                export_profdata_config_flags,
                compile_test_index_flags,
            )?;
        }
        LowLevelCommand::RunAnalysisWithTestIndex {
            index,
            algo: AlgoArgs { algo, commit },
        } => {
            run_analysis_with_test_index(index, algo, commit)?;
        }
        LowLevelCommand::IndexesTouchSameFilesReport {
            index1,
            index2,
            action,
        } => {
            run_indexes_touch_same_files_report(index1, index2, action)?;
        }
    }

    Ok(())
}

fn analyze_single_test(
    difftest: &mut Difftest,
    force: bool,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    analysis_index: &AnalysisIndex,
    resolver: Option<&DiscoverIndexPathResolver>,
) -> CargoDifftestsResult<AnalysisResult> {
    let mut analysis_cx = match analysis_index.index_strategy {
        AnalysisIndexStrategy::Never => {
            difftest.merge_profraw_files_into_profdata(force)?;

            difftest.start_analysis(export_profdata_config_flags.config())?
        }
        AnalysisIndexStrategy::Always => {
            'l: {
                if difftest.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest(difftest)?;
                }

                difftest.merge_profraw_files_into_profdata(force)?;

                let config =
                    compile_test_index_config(analysis_index.compile_test_index_flags.clone())?;

                let test_index_data = difftest
                    .compile_test_index_data(export_profdata_config_flags.config(), config)?;

                if let Some(p) = resolver.and_then(|r| r.resolve(difftest.dir())) {
                    let parent = p.parent().unwrap();
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                    test_index_data.write_to_file(&p)?;
                }

                AnalysisContext::from_index(test_index_data)
            }
        }
        AnalysisIndexStrategy::AlwaysAndClean => {
            'l: {
                if difftest.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest(difftest)?;
                }

                difftest.merge_profraw_files_into_profdata(force)?;

                let config =
                    compile_test_index_config(analysis_index.compile_test_index_flags.clone())?;

                let test_index_data = difftest
                    .compile_test_index_data(export_profdata_config_flags.config(), config)?;

                if let Some(p) = resolver.and_then(|r| r.resolve(difftest.dir())) {
                    let parent = p.parent().unwrap();
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                    test_index_data.write_to_file(&p)?;

                    difftest.clean()?;
                }

                AnalysisContext::from_index(test_index_data)
            }
        }
        AnalysisIndexStrategy::IfAvailable => {
            'l: {
                if difftest.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest(difftest)?;
                }

                difftest.merge_profraw_files_into_profdata(force)?;

                difftest.start_analysis(export_profdata_config_flags.config())?
            }
        }
    };

    analysis_cx.run(&AnalysisConfig {
        dirty_algorithm: algo.convert(commit),
        error_on_invalid_config: true,
    })?;

    let r = analysis_cx.finish_analysis();

    Ok(r)
}

fn analyze_single_group(
    group: &mut GroupDifftestGroup,
    force: bool,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    analysis_index: &AnalysisIndex,
    resolver: Option<&DiscoverIndexPathResolver>,
) -> CargoDifftestsResult<AnalysisResult> {
    let mut analysis_cx = match analysis_index.index_strategy {
        AnalysisIndexStrategy::Never => {
            group.merge_profraws(force)?;

            group.start_analysis(true)?
        }
        AnalysisIndexStrategy::Always => {
            'l: {
                if group.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest_group(group)?;
                }

                group.merge_profraws(force)?;

                let config =
                    compile_test_index_config(analysis_index.compile_test_index_flags.clone())?;

                let test_index_data = group.compile_test_index_data(config)?;

                if let Some(p) = resolver.and_then(|r| r.resolve(group.dir())) {
                    let parent = p.parent().unwrap();
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                    test_index_data.write_to_file(&p)?;
                }

                AnalysisContext::from_index(test_index_data)
            }
        }
        AnalysisIndexStrategy::AlwaysAndClean => {
            'l: {
                if group.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest_group(group)?;
                }

                group.merge_profraws(force)?;

                let config =
                    compile_test_index_config(analysis_index.compile_test_index_flags.clone())?;

                let test_index_data = group.compile_test_index_data(config)?;

                if let Some(p) = resolver.and_then(|r| r.resolve(group.dir())) {
                    let parent = p.parent().unwrap();
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                    test_index_data.write_to_file(&p)?;

                    group.clean()?;
                }

                AnalysisContext::from_index(test_index_data)
            }
        }
        AnalysisIndexStrategy::IfAvailable => {
            'l: {
                if group.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest_group(group)?;
                }

                group.merge_profraws(force)?;

                group.start_analysis(true)?
            }
        }
    };

    analysis_cx.run(&AnalysisConfig {
        dirty_algorithm: algo.convert(commit),
        error_on_invalid_config: true,
    })?;

    let r = analysis_cx.finish_analysis();

    Ok(r)
}

fn run_analyze(
    dir: PathBuf,
    force: bool,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    root: Option<PathBuf>,
    analysis_index: AnalysisIndex,
) -> CargoDifftestsResult {
    let resolver = analysis_index.index_resolver(root)?;

    let mut difftest = Difftest::discover_from(dir, resolver.as_ref())?;

    let r = analyze_single_test(
        &mut difftest,
        force,
        algo,
        commit,
        export_profdata_config_flags,
        &analysis_index,
        resolver.as_ref(),
    )?;

    display_analysis_result(r);

    Ok(())
}

pub fn run_analyze_all(
    dir: PathBuf,
    force: bool,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    analysis_index: AnalysisIndex,
    ignore_incompatible: bool,
    action_args: AnalyzeAllActionArgs,
) -> CargoDifftestsResult {
    let resolver = analysis_index.index_resolver(Some(dir.clone()))?;
    let discovered =
        discover_difftests(dir, analysis_index.index_root.clone(), ignore_incompatible)?;

    let mut results = vec![];

    for mut difftest in discovered {
        let r = analyze_single_test(
            &mut difftest,
            force,
            algo,
            commit,
            export_profdata_config_flags.clone(),
            &analysis_index,
            resolver.as_ref(),
        )?;

        let result = AnalyzeAllSingleTestGroup {
            test_desc: vec![difftest.load_test_desc()?],
            difftest: Some(difftest),
            difftest_group: None,
            verdict: r.into(),
        };

        results.push(result);
    }

    action_args.perform_for(&results)?;

    Ok(())
}

fn discover_indexes_to_vec(
    index_root: &Path,
    indexes: &mut Vec<TestIndex>,
) -> CargoDifftestsResult {
    for entry in fs::read_dir(index_root)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            discover_indexes_to_vec(&path, indexes)?;
        } else {
            let index = TestIndex::read_from_file(&path)?;
            indexes.push(index);
        }
    }

    Ok(())
}

pub fn run_analyze_all_from_index(
    index_root: PathBuf,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    action_args: AnalyzeAllActionArgs,
) -> CargoDifftestsResult {
    let indexes = {
        let mut indexes = vec![];
        discover_indexes_to_vec(&index_root, &mut indexes)?;
        indexes
    };

    let mut results = vec![];

    for index in indexes {
        let test_desc = index.test_desc.clone();

        let r = {
            let mut analysis_cx = AnalysisContext::from_index(index);
            analysis_cx.run(&AnalysisConfig {
                dirty_algorithm: algo.convert(commit),
                error_on_invalid_config: true,
            })?;
            analysis_cx.finish_analysis()
        };

        let result = AnalyzeAllSingleTestGroup {
            test_desc,
            difftest: None,
            difftest_group: None,
            verdict: r.into(),
        };

        results.push(result);
    }

    action_args.perform_for(&results)?;

    Ok(())
}

fn main_impl() -> CargoDifftestsResult {
    pretty_env_logger::init_custom_env("CARGO_DIFFTESTS_LOG");
    let CargoApp::Difftests { app } = CargoApp::parse();

    match app {
        App::DiscoverDifftests {
            dir,
            index_root,
            ignore_incompatible,
        } => {
            run_discover_difftests(dir, index_root, ignore_incompatible)?;
        }
        App::Analyze {
            dir,
            root,
            force,
            algo: AlgoArgs { algo, commit },
            export_profdata_config_flags,
            analysis_index,
        } => {
            run_analyze(
                dir.dir,
                force,
                algo,
                commit,
                export_profdata_config_flags,
                root,
                analysis_index,
            )?;
        }
        App::AnalyzeGroup {
            dir,
            force,
            algo,
            other_binaries,
            analysis_index,
            root,
        } => {
            let resolver = analysis_index.index_resolver(root)?;
            let mut group = cargo_difftests::group_difftest::index_group(
                dir,
                other_binaries.other_binaries,
                resolver.as_ref(),
            )?;

            let r = analyze_single_group(
                &mut group,
                force,
                algo.algo,
                algo.commit,
                &analysis_index,
                resolver.as_ref(),
            )?;

            display_analysis_result(r);
        }
        App::AnalyzeAll {
            dir,
            force,
            algo: AlgoArgs { algo, commit },
            export_profdata_config_flags,
            analysis_index,
            ignore_incompatible,
            action_args,
        } => {
            run_analyze_all(
                dir,
                force,
                algo,
                commit,
                export_profdata_config_flags,
                analysis_index,
                ignore_incompatible,
                action_args,
            )?;
        }
        App::AnalyzeAllFromIndex {
            index_root,
            algo: AlgoArgs { algo, commit },
            action_args,
        } => {
            run_analyze_all_from_index(index_root, algo, commit, action_args)?;
        }
        App::LowLevel { cmd } => {
            run_low_level_cmd(cmd)?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = main_impl() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

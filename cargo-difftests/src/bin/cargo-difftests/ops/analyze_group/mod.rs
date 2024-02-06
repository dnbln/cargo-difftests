use std::path::PathBuf;

use cargo_difftests::bin_context::CargoDifftestsContext;
use clap::Parser;

use crate::{
    cli_core::{AlgoArgs, AnalysisIndex, IgnoreRegistryFilesFlag, OtherBinaries},
    CargoDifftestsResult,
};

use crate::ops::core::{analyze_single_group, display_analysis_result};

#[derive(Debug, Parser)]
pub struct AnalyzeGroupCommand {
    /// The root directory where the difftest group was stored.
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

    #[clap(flatten)]
    ignore_registry_files: IgnoreRegistryFilesFlag,
}

impl AnalyzeGroupCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_analyze_group(
            ctxt,
            self.root,
            self.force,
            self.algo,
            self.other_binaries,
            self.analysis_index,
            self.dir,
            self.ignore_registry_files,
        )
    }
}

fn run_analyze_group(
    ctxt: &CargoDifftestsContext,
    root: Option<PathBuf>,
    force: bool,
    algo: AlgoArgs,
    other_binaries: OtherBinaries,
    analysis_index: AnalysisIndex,
    dir: PathBuf,
    ignore_registry_files: IgnoreRegistryFilesFlag,
) -> CargoDifftestsResult {
    let resolver = analysis_index.index_resolver(root)?;
    let mut group = cargo_difftests::group_difftest::index_group(
        dir,
        other_binaries.other_binaries,
        resolver.as_ref(),
    )?;

    let r = analyze_single_group(
        &ctxt,
        &mut group,
        force,
        algo.algo,
        algo.commit,
        &analysis_index,
        resolver.as_ref(),
        ignore_registry_files,
    )?;

    display_analysis_result(r);

    Ok(())
}

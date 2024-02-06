use std::path::PathBuf;

use cargo_difftests::{bin_context::CargoDifftestsContext, difftest::Difftest};
use clap::Parser;

use crate::{
    cli_core::{
        AlgoArgs, AnalysisIndex, DifftestDir, DifftestsRoot, DirtyAlgorithm, ExportProfdataConfigFlags, IgnoreRegistryFilesFlag
    },
    CargoDifftestsResult,
};

use crate::ops::core::{analyze_single_test, display_analysis_result};

#[derive(Parser, Debug)]
pub struct AnalyzeCommand {
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

    #[clap(flatten)]
    root: DifftestsRoot,

    #[clap(flatten)]
    ignore_registry_files: IgnoreRegistryFilesFlag,
}

impl AnalyzeCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> crate::CargoDifftestsResult {
        run_analyze(
            ctxt,
            self.dir.dir,
            self.force,
            self.algo.algo,
            self.algo.commit,
            self.export_profdata_config_flags,
            self.root.root,
            self.analysis_index,
            self.ignore_registry_files,
        )
    }
}

fn run_analyze(
    ctxt: &CargoDifftestsContext,
    dir: PathBuf,
    force: bool,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    root: Option<PathBuf>,
    analysis_index: AnalysisIndex,
    ignore_registry_files: IgnoreRegistryFilesFlag,
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
        ignore_registry_files,
    )?;

    display_analysis_result(r);

    Ok(())
}

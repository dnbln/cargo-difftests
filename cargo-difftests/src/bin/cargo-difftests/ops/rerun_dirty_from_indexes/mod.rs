use std::path::PathBuf;

use cargo_difftests::{bin_context::CargoDifftestsContext, AnalyzeAllSingleTest};
use clap::Parser;

use crate::{
    cli_core::{AlgoArgs, AnalysisIndex, AnalyzeAllActionArgs, AnalyzeAllActionKind, DifftestsRoot, DifftestsRootRequired, ExportProfdataConfigFlags, IgnoreRegistryFilesFlag, RerunRunner},
    CargoDifftestsResult,
};

use super::{analyze_all_from_index, core::discover_indexes_to_vec};

#[derive(Parser, Debug)]
pub struct RerunDirtyFromIndexesCommand {
    #[clap(long)]
    index_root: PathBuf,
    #[clap(flatten)]
    runner: RerunRunner,

    #[clap(flatten)]
    algo_args: AlgoArgs,
}

impl RerunDirtyFromIndexesCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        analyze_all_from_index::AnalyzeAllFromIndexCommand {
            index_root: self.index_root,
            algo: self.algo_args,
            action_args: AnalyzeAllActionArgs {
                action: AnalyzeAllActionKind::RerunDirty,
                runner: self.runner,
            },
        }
        .run(ctxt)
    }
}

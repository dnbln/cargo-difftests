use std::path::PathBuf;

use cargo_difftests::{analysis::{AnalysisConfig, AnalysisContext}, bin_context::CargoDifftestsContext};
use clap::Parser;

use crate::{cli_core::{AlgoArgs, DirtyAlgorithm}, ops::core::display_analysis_result, CargoDifftestsResult};

#[derive(Parser, Debug)]
pub struct RunAnalysisWithTestIndexCommand {
    /// The path to the test index.
    #[clap(long)]
    index: PathBuf,
    #[clap(flatten)]
    algo: AlgoArgs,
}

impl RunAnalysisWithTestIndexCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_analysis_with_test_index(self.index, self.algo.algo, self.algo.commit)
    }
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

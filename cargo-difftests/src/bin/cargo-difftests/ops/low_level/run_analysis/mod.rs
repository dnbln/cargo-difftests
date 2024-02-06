use std::path::PathBuf;

use cargo_difftests::{analysis::AnalysisConfig, bin_context::CargoDifftestsContext, difftest::{Difftest, ExportProfdataConfig}};
use clap::Parser;

use crate::{cli_core::{AlgoArgs, DifftestDir, DirtyAlgorithm}, ops::core::display_analysis_result, CargoDifftestsResult};

#[derive(Parser, Debug)]
pub struct RunAnalysisCommand {
    #[clap(flatten)]
    dir: DifftestDir,
    #[clap(flatten)]
    algo: AlgoArgs,
}

impl RunAnalysisCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_analysis(self.dir.dir, self.algo.algo, self.algo.commit)
    }
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

use std::{ffi::OsString, path::PathBuf};

use cargo_difftests::{
    analysis::{AnalysisConfig, AnalysisContext},
    bin_context::CargoDifftestsContext,
    AnalyzeAllSingleTest,
};
use clap::Parser;
use prodash::unit;

use crate::{
    cli_core::{AlgoArgs, AnalysisIndex, AnalyzeAllActionArgs, DifftestsRootRequired, DirtyAlgorithm, ExportProfdataConfigFlags, IgnoreRegistryFilesFlag},
    ops::core::discover_indexes_to_vec,
    CargoDifftestsResult,
};

#[derive(Parser, Debug)]
pub struct AnalyzeAllFromIndexCommand {
    /// The root directory where all the index files are stored.
    #[clap(long)]
    pub(crate) index_root: PathBuf,
    #[clap(flatten)]
    pub(crate) algo: AlgoArgs,
    #[clap(flatten)]
    pub(crate) action_args: AnalyzeAllActionArgs,
}

impl AnalyzeAllFromIndexCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_analyze_all_from_index(
            &ctxt,
            self.index_root,
            self.algo.algo,
            self.algo.commit,
            self.action_args,
        )
    }
}

fn run_analyze_all_from_index(
    ctxt: &CargoDifftestsContext,
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

    let mut pb = ctxt.new_child("Analyzing tests");
    pb.init(Some(indexes.len()), Some(unit::label("indexes")));

    let mut results = vec![];

    for index in indexes {
        let test_desc = index.test_info.clone();

        let r = {
            let mut analysis_cx = AnalysisContext::from_index(index);
            analysis_cx.run(&AnalysisConfig {
                dirty_algorithm: algo.convert(commit),
                error_on_invalid_config: true,
            })?;
            analysis_cx.finish_analysis()
        };

        let result = AnalyzeAllSingleTest {
            test_info: test_desc,
            difftest: None,
            verdict: r.into(),
        };

        results.push(result);
        pb.inc();
    }

    pb.done("done");

    action_args.perform_for(ctxt, &results)?;

    Ok(())
}

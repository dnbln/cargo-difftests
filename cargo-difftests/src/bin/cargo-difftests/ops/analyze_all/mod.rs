use std::path::PathBuf;

use cargo_difftests::{bin_context::CargoDifftestsContext, AnalyzeAllSingleTest};
use clap::Parser;
use prodash::unit;

use crate::{
    cli_core::{
        AlgoArgs, AnalysisIndex, AnalyzeAllActionArgs, DifftestsRootDir, DirtyAlgorithm,
        ExportProfdataConfigFlags, IgnoreRegistryFilesFlag,
    },
    CargoDifftestsResult,
};

use crate::ops::core::{analyze_single_test, discover_difftests};

#[derive(Parser, Debug)]
pub struct AnalyzeAllCommand {
    #[clap(flatten)]
    dir: DifftestsRootDir,
    #[clap(flatten)]
    ignore_registry_files: IgnoreRegistryFilesFlag,
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
}

impl AnalyzeAllCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_analyze_all(
            ctxt,
            self.dir.dir,
            self.force,
            self.algo.algo,
            self.algo.commit,
            self.export_profdata_config_flags,
            self.analysis_index,
            self.ignore_incompatible,
            self.action_args,
            self.ignore_registry_files,
        )
    }
}

fn run_analyze_all(
    ctxt: &CargoDifftestsContext,
    dir: PathBuf,
    force: bool,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    analysis_index: AnalysisIndex,
    ignore_incompatible: bool,
    action_args: AnalyzeAllActionArgs,
    ignore_registry_files: IgnoreRegistryFilesFlag,
) -> CargoDifftestsResult {
    let resolver = analysis_index.index_resolver(Some(dir.clone()))?;
    let discovered =
        discover_difftests(dir, analysis_index.index_root.clone(), ignore_incompatible)?;

    let mut results = vec![];

    let mut pb = ctxt.new_child("Analyzing tests");
    pb.init(Some(discovered.len()), Some(unit::label("difftests")));

    for mut difftest in discovered.into_iter() {
        let r = analyze_single_test(
            &mut difftest,
            force,
            algo,
            commit,
            export_profdata_config_flags.clone(),
            &analysis_index,
            resolver.as_ref(),
            ignore_registry_files,
        )?;

        let result = AnalyzeAllSingleTest {
            test_info: difftest.test_info()?,
            difftest: Some(difftest),
            verdict: r.into(),
        };

        results.push(result);

        pb.inc();
    }

    pb.done("done");

    action_args.perform_for(ctxt, &results)?;

    Ok(())
}

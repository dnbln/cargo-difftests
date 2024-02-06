use cargo_difftests::bin_context::CargoDifftestsContext;
use clap::Parser;

use crate::CargoDifftestsResult;

mod compile_test_index;
mod export_profdata;
mod indexes_touch_same_files_report;
mod merge_profdata;
mod run_analysis;
mod run_analysis_with_test_index;
mod test_client_compile_test_index_and_clean;

#[derive(Parser, Debug)]
pub enum LowLevelCommand {
    /// Run the `llvm-profdata merge` command, to merge all
    /// the `.profraw` files from a difftest directory into
    /// a single `.profdata` file.
    MergeProfdata {
        #[clap(flatten)]
        cmd: merge_profdata::MergeProfdataCommand,
    },
    /// Run the `llvm-cov export` command, to export the
    /// `.profdata` file into a `.json` file that can be later
    /// used for analysis.
    ExportProfdata {
        #[clap(flatten)]
        cmd: export_profdata::ExportProfdataCommand,
    },
    /// Run the analysis for a single difftest directory.
    RunAnalysis {
        #[clap(flatten)]
        cmd: run_analysis::RunAnalysisCommand,
    },
    /// Compile a test index for a single difftest directory.
    CompileTestIndex {
        #[clap(flatten)]
        cmd: compile_test_index::CompileTestIndexCommand,
    },
    /// Compile a test index for a single difftest directory, then
    /// clean the difftest directory. (internal: only used from the testclient).
    TestClientCompileTestIndexAndClean {
        #[clap(flatten)]
        cmd: test_client_compile_test_index_and_clean::TestClientCompileIndexAndCleanCommand,
    },
    /// Runs the analysis for a single test index.
    RunAnalysisWithTestIndex {
        #[clap(flatten)]
        cmd: run_analysis_with_test_index::RunAnalysisWithTestIndexCommand,
    },
    /// Compare two test indexes, by the files that they "touch"
    /// (have regions that have an execution count > 0).
    IndexesTouchSameFilesReport {
        #[clap(flatten)]
        cmd: indexes_touch_same_files_report::IndexesTouchSameFilesReportCommand,
    },
}

impl LowLevelCommand {
    pub(crate) fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_low_level_cmd(ctxt, self)
    }
}

fn run_low_level_cmd(ctxt: &CargoDifftestsContext, cmd: LowLevelCommand) -> CargoDifftestsResult {
    match cmd {
        LowLevelCommand::MergeProfdata { cmd } => {
            cmd.run(ctxt)?;
        }
        LowLevelCommand::ExportProfdata { cmd } => {
            cmd.run(ctxt)?;
        }
        LowLevelCommand::RunAnalysis { cmd } => {
            cmd.run(ctxt)?;
        }
        LowLevelCommand::CompileTestIndex { cmd } => {
            cmd.run(ctxt)?;
        }
        LowLevelCommand::TestClientCompileTestIndexAndClean { cmd } => {
            cmd.run(ctxt)?;
        }
        LowLevelCommand::RunAnalysisWithTestIndex { cmd } => {
            cmd.run(ctxt)?;
        }
        LowLevelCommand::IndexesTouchSameFilesReport { cmd } => {
            cmd.run(ctxt)?;
        }
    }

    Ok(())
}

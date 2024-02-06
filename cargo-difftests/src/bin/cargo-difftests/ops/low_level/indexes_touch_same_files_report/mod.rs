use std::path::PathBuf;

use cargo_difftests::{bin_context::CargoDifftestsContext, index_data::TestIndex};
use clap::Parser;

use crate::{cli_core::IndexesTouchSameFilesReportAction, CargoDifftestsResult};

#[derive(Parser, Debug)]
pub struct IndexesTouchSameFilesReportCommand {
    /// The first index to compare.
    index1: PathBuf,
    /// The second index to compare.
    index2: PathBuf,
    /// The action to take for the report.
    #[clap(long, default_value_t = Default::default())]
    action: IndexesTouchSameFilesReportAction,
}

impl IndexesTouchSameFilesReportCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_indexes_touch_same_files_report(self.index1, self.index2, self.action)
    }
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

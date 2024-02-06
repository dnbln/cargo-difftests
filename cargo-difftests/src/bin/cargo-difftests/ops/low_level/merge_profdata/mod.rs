use std::path::PathBuf;

use cargo_difftests::{bin_context::CargoDifftestsContext, difftest::Difftest};
use clap::Parser;

use crate::{cli_core::DifftestDir, CargoDifftestsResult};

#[derive(Parser, Debug)]
pub struct MergeProfdataCommand {
    #[clap(flatten)]
    dir: DifftestDir,
    /// Whether to force the merge.
    ///
    /// If this flag is not passed, and the `.profdata` file
    /// already exists, the merge will not be run.
    #[clap(long)]
    force: bool,
}

impl MergeProfdataCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_merge_profdata(self.dir.dir, self.force)
    }
}

fn run_merge_profdata(dir: PathBuf, force: bool) -> CargoDifftestsResult {
    // we do not need the index resolver here, because we are not going to use the index
    let mut discovered = Difftest::discover_from(dir, None)?;

    discovered.merge_profraw_files_into_profdata(force)?;

    Ok(())
}


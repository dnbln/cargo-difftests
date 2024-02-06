use std::path::PathBuf;

use anyhow::bail;
use cargo_difftests::{bin_context::CargoDifftestsContext, difftest::Difftest};
use clap::Args;

use crate::{
    cli_core::{DifftestDir, ExportProfdataConfigFlags, IgnoreRegistryFilesFlag},
    CargoDifftestsResult,
};

#[derive(Args, Debug)]
pub struct ExportProfdataCommand {
    #[clap(flatten)]
    dir: DifftestDir,
    #[clap(flatten)]
    export_profdata_config_flags: ExportProfdataConfigFlags,
    #[clap(flatten)]
    ignore_registry_files: IgnoreRegistryFilesFlag,
}

impl ExportProfdataCommand {
    pub(crate) fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_export_profdata(
            ctxt,
            self.dir.dir,
            self.export_profdata_config_flags,
            self.ignore_registry_files,
        )
    }
}

fn run_export_profdata(
    ctxt: &CargoDifftestsContext,
    dir: PathBuf,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    ignore_registry_files: IgnoreRegistryFilesFlag,
) -> CargoDifftestsResult {
    // we do not need the index resolver here, because we are not going to use the index
    let discovered = Difftest::discover_from(dir, None)?;

    if !discovered.has_profdata() {
        bail!("difftest directory does not have a .profdata file");
    }

    let coverage =
        discovered.export_profdata(export_profdata_config_flags.config(ignore_registry_files))?;

    let s = serde_json::to_string(&coverage)?;

    println!("{s}");

    Ok(())
}

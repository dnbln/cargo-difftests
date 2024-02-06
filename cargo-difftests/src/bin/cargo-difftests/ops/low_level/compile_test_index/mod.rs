use std::path::PathBuf;

use cargo_difftests::{bin_context::CargoDifftestsContext, difftest::Difftest};
use clap::Parser;

use crate::{cli_core::{CompileTestIndexFlags, DifftestDir, ExportProfdataConfigFlags, IgnoreRegistryFilesFlag}, ops::core::compile_test_index_config, CargoDifftestsResult};

#[derive(Parser, Debug)]
pub struct CompileTestIndexCommand {
    #[clap(flatten)]
    dir: DifftestDir,
    /// The output file to write the index to.
    #[clap(short, long)]
    output: PathBuf,
    #[clap(flatten)]
    export_profdata_config_flags: ExportProfdataConfigFlags,
    #[clap(flatten)]
    compile_test_index_flags: CompileTestIndexFlags,
    #[clap(flatten)]
    ignore_registry_files: IgnoreRegistryFilesFlag,
}
impl CompileTestIndexCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_compile_test_index(
            self.dir.dir,
            self.output,
            self.export_profdata_config_flags,
            self.compile_test_index_flags,
            self.ignore_registry_files,
        )
    }
}

fn run_compile_test_index(
    dir: PathBuf,
    output: PathBuf,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    compile_test_index_flags: CompileTestIndexFlags,
    ignore_registry_files: IgnoreRegistryFilesFlag,
) -> CargoDifftestsResult {
    let discovered = Difftest::discover_from(dir, None)?;
    assert!(discovered.has_profdata());

    let config = compile_test_index_config(compile_test_index_flags, ignore_registry_files)?;

    let result = discovered.compile_test_index_data(
        export_profdata_config_flags.config(ignore_registry_files),
        config,
    )?;

    result.write_to_file(&output)?;

    Ok(())
}




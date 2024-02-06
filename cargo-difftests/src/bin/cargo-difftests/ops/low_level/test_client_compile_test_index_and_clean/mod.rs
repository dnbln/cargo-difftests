use std::{fs, path::PathBuf};

use cargo_difftests::{bin_context::CargoDifftestsContext, difftest::{Difftest, DiscoverIndexPathResolver}};
use clap::Parser;

use crate::{cli_core::{CompileTestIndexFlags, DifftestDir, ExportProfdataConfigFlags, IgnoreRegistryFilesFlag, IndexPathOrResolve}, ops::core::{compile_test_index_config, resolver_for_index_root}, CargoDifftestsResult};

#[derive(Parser, Debug)]
pub struct TestClientCompileIndexAndCleanCommand {
    #[clap(flatten)]
    dir: DifftestDir,
    /// The output file to write the index to.
    #[clap(short, long)]
    output: IndexPathOrResolve,
    #[clap(flatten)]
    export_profdata_config_flags: ExportProfdataConfigFlags,
    #[clap(flatten)]
    compile_test_index_flags: CompileTestIndexFlags,
    #[clap(flatten)]
    ignore_registry_files: IgnoreRegistryFilesFlag,

    /// The root directory where all the difftests were stored.
    #[clap(long, required_if_eq("output", "resolve"))]
    root: Option<PathBuf>,
    /// The directory where the index files were stored, if any.
    #[clap(long, required_if_eq("output", "resolve"))]
    index_root: Option<PathBuf>,
}

impl TestClientCompileIndexAndCleanCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_test_client_compile_test_index_and_clean(
            self.dir.dir,
            self.output,
            self.export_profdata_config_flags,
            self.compile_test_index_flags,
            self.ignore_registry_files,
            self.root,
            self.index_root,
        )
    }
}

fn run_test_client_compile_test_index_and_clean(
    dir: PathBuf,
    output: IndexPathOrResolve,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    compile_test_index_flags: CompileTestIndexFlags,
    ignore_registry_files: IgnoreRegistryFilesFlag,
    root: Option<PathBuf>,
    index_root: Option<PathBuf>,
) -> CargoDifftestsResult {
    let index_resolver = match output {
        IndexPathOrResolve::Resolve => {
            resolver_for_index_root(root.as_ref().map(PathBuf::as_path).unwrap(), index_root)
                .unwrap()
        }
        IndexPathOrResolve::Path(p) => DiscoverIndexPathResolver::Custom {
            f: Box::new(move |_| Some(p.clone())),
        },
    };

    let mut discovered = Difftest::discover_from(dir.clone(), Some(&index_resolver))?;
    discovered.merge_profraw_files_into_profdata(false)?;

    assert!(discovered.has_profdata());

    let config = compile_test_index_config(compile_test_index_flags, ignore_registry_files)?;

    let result = discovered.compile_test_index_data(
        export_profdata_config_flags.config(ignore_registry_files),
        config,
    )?;

    let output = index_resolver.resolve(&dir).unwrap();

    if let Some(parent) = output.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    result.write_to_file(&output)?;

    discovered.clean()?;

    Ok(())
}

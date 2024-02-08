use std::path::PathBuf;

use cargo_difftests::{bin_context::CargoDifftestsContext, difftest::Difftest};
use clap::Parser;
use prodash::unit;

use crate::{
    cli_core::{AnalysisIndex, DifftestsRoot, DifftestsRootRequired, ExportProfdataConfigFlags, IgnoreRegistryFilesFlag},
    CargoDifftestsResult,
};

use super::core::{collect_test_harnesses, compile_test_index_config};

#[derive(Parser, Debug)]
pub struct CollectProfilingDataCommand {
    #[clap(flatten)]
    root: DifftestsRootRequired,

    #[clap(flatten)]
    export_profdata_args: ExportProfdataConfigFlags,

    #[clap(flatten)]
    index_compilation_args: AnalysisIndex,

    #[clap(flatten)]
    ignore_registry_files: IgnoreRegistryFilesFlag,

    #[clap(long)]
    filter: Option<String>,

    #[clap(long)]
    exact: bool,
}

impl CollectProfilingDataCommand {
    pub fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        run_collect_profiling_data(
            ctxt,
            self.root.root,
            self.export_profdata_args,
            self.index_compilation_args.compile_index,
            self.index_compilation_args,
            self.ignore_registry_files,
            self.filter,
            self.exact,
        )
    }
}

fn run_collect_profiling_data(
    ctxt: &CargoDifftestsContext,
    root: PathBuf,
    export_profdata_args: ExportProfdataConfigFlags,
    compile_index: bool,
    index_compilation_args: AnalysisIndex,
    ignore_registry_files: IgnoreRegistryFilesFlag,
    filter: Option<String>,
    exact: bool,
) -> CargoDifftestsResult {
    let index_resolver = index_compilation_args.index_resolver(Some(root.clone()))?;

    let mut pb = ctxt.new_child("Collecting profiling data for tests");
    pb.init(Some(1), None);

    let test_harnesses = collect_test_harnesses()?;

    let mut test_harnesses_pb = pb.add_child("Collecting tests");
    test_harnesses_pb.init(
        Some(test_harnesses.len()),
        Some(unit::label("test harnesses")),
    );

    let mut tests = vec![];

    for test_harness in test_harnesses {
        let mut t = test_harness.list_tests()?;

        if let Some(filter) = filter.as_ref() {
            t.retain(|it| {
                if exact {
                    it.get_name() == filter
                } else {
                    it.get_name().contains(filter)
                }
            });
        }

        tests.extend(t);

        test_harnesses_pb.inc();
    }

    test_harnesses_pb.done("done");

    let mut tests_pb = pb.add_child("Collecting profiling data");

    tests_pb.init(Some(tests.len()), Some(unit::label("tests")));

    let export_profdata_config = export_profdata_args.config(ignore_registry_files);

    for test in tests {
        let harness_name = test.get_harness_name().clone();
        let name = test.get_name().clone();

        let difftest_dir = root.join(&harness_name).join(&name);

        let mut test_pb = tests_pb.add_child(&format!("{}::{}", harness_name, name));
        test_pb.init(Some(1), Some(unit::label("test")));

        if difftest_dir.exists() {
            std::fs::remove_dir_all(&difftest_dir)?;
        }

        std::fs::create_dir_all(&difftest_dir)?;

        std::fs::write(
            difftest_dir.join(cargo_difftests_core::CARGO_DIFFTESTS_TEST_BINARY_FILENAME),
            test.get_harness_path().to_str().unwrap(),
        )?;

        std::fs::write(
            difftest_dir.join(cargo_difftests_core::CARGO_DIFFTESTS_TEST_NAME_FILENAME),
            &name,
        )?;

        std::fs::write(
            difftest_dir.join(cargo_difftests_core::CARGO_DIFFTESTS_VERSION_FILENAME),
            env!("CARGO_PKG_VERSION"),
        )?;

        match test.run_test_and_collect_profiling_data(&difftest_dir) {
            Ok(_) => {
                if compile_index {
                    if let Some(index_resolver) = index_resolver.as_ref() {
                        let mut difftest =
                            Difftest::discover_from(difftest_dir.clone(), Some(index_resolver))?;

                        difftest.merge_profraw_files_into_profdata(false)?;
                        let index_data_compiler_config = compile_test_index_config(
                            index_compilation_args.compile_test_index_flags.clone(),
                            ignore_registry_files,
                        )?;
                        let index_data = difftest.compile_test_index_data(
                            export_profdata_config.clone(),
                            index_data_compiler_config,
                        )?;

                        if let Some(path) = index_resolver.resolve(&difftest_dir) {
                            if let Some(p) = path.parent() {
                                if !p.exists() {
                                    std::fs::create_dir_all(p)?;
                                }
                            }
                            index_data.write_to_file(&path)?;
                        }
                    }
                }

                test_pb.done("done");
            }
            Err(e) => {
                test_pb.fail(&format!("Failed to run test: {}", e));
                tests_pb.fail("Failed to run tests");
                pb.fail("Failed");
                return Err(e);
            }
        }

        tests_pb.inc();
    }

    Ok(())
}

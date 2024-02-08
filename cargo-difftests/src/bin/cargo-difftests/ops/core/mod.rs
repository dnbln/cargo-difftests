use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context};
use cargo_difftests::{
    analysis::{file_is_from_cargo_registry, AnalysisConfig, AnalysisContext, AnalysisResult},
    difftest::{Difftest, DiscoverIndexPathResolver},
    index_data::{IndexDataCompilerConfig, IndexSize, TestIndex},
};
use log::{error, warn};

use crate::{
    cli_core::{
        AnalysisIndex, AnalysisIndexStrategy, CompileTestIndexFlags, DirtyAlgorithm,
        ExportProfdataConfigFlags, FlattenFilesTarget, IgnoreRegistryFilesFlag,
    },
    CargoDifftestsResult,
};

pub fn analyze_single_test(
    difftest: &mut Difftest,
    force: bool,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    export_profdata_config_flags: ExportProfdataConfigFlags,
    analysis_index: &AnalysisIndex,
    resolver: Option<&DiscoverIndexPathResolver>,
    ignore_registry_files: IgnoreRegistryFilesFlag,
) -> CargoDifftestsResult<AnalysisResult> {
    let mut analysis_cx = match analysis_index.index_strategy {
        AnalysisIndexStrategy::Never => {
            difftest.merge_profraw_files_into_profdata(force)?;

            difftest.start_analysis(export_profdata_config_flags.config(ignore_registry_files))?
        }
        AnalysisIndexStrategy::Always => {
            'l: {
                if difftest.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest(difftest)?;
                }

                difftest.merge_profraw_files_into_profdata(force)?;

                let config = compile_test_index_config(
                    analysis_index.compile_test_index_flags.clone(),
                    ignore_registry_files,
                )?;

                let test_index_data = difftest.compile_test_index_data(
                    export_profdata_config_flags.config(ignore_registry_files),
                    config,
                )?;

                if let Some(p) = resolver.and_then(|r| r.resolve(difftest.dir())) {
                    let parent = p.parent().unwrap();
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                    test_index_data.write_to_file(&p)?;
                }

                AnalysisContext::from_index(test_index_data)
            }
        }
        AnalysisIndexStrategy::AlwaysAndClean => {
            'l: {
                if difftest.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest(difftest)?;
                }

                difftest.merge_profraw_files_into_profdata(force)?;

                let config = compile_test_index_config(
                    analysis_index.compile_test_index_flags.clone(),
                    ignore_registry_files,
                )?;

                let test_index_data = difftest.compile_test_index_data(
                    export_profdata_config_flags.config(ignore_registry_files),
                    config,
                )?;

                if let Some(p) = resolver.and_then(|r| r.resolve(difftest.dir())) {
                    let parent = p.parent().unwrap();
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                    test_index_data.write_to_file(&p)?;

                    difftest.clean()?;
                }

                AnalysisContext::from_index(test_index_data)
            }
        }
        AnalysisIndexStrategy::IfAvailable => {
            'l: {
                if difftest.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest(difftest)?;
                }

                difftest.merge_profraw_files_into_profdata(force)?;

                difftest
                    .start_analysis(export_profdata_config_flags.config(ignore_registry_files))?
            }
        }
    };

    analysis_cx.run(&AnalysisConfig {
        dirty_algorithm: algo.convert(commit),
        error_on_invalid_config: true,
    })?;

    let r = analysis_cx.finish_analysis();

    Ok(r)
}

pub fn discover_indexes_to_vec(
    index_root: &Path,
    indexes: &mut Vec<TestIndex>,
) -> CargoDifftestsResult {
    for entry in fs::read_dir(index_root)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            discover_indexes_to_vec(&path, indexes)?;
        } else {
            let index = TestIndex::read_from_file(&path)?;
            indexes.push(index);
        }
    }

    Ok(())
}

pub fn compile_test_index_config(
    compile_test_index_flags: CompileTestIndexFlags,
    ignore_registry_files: IgnoreRegistryFilesFlag,
) -> CargoDifftestsResult<IndexDataCompilerConfig> {
    let flatten_root = match compile_test_index_flags.flatten_files_to {
        Some(FlattenFilesTarget::RepoRoot) => {
            let repo = git2::Repository::open_from_env()?;
            let root = repo.workdir().context("repo has no workdir")?;
            Some(root.to_path_buf())
        }
        None => None,
    };

    let config = IndexDataCompilerConfig {
        ignore_registry_files: true,
        index_filename_converter: Box::new(move |path| {
            let p = match &flatten_root {
                Some(root) => path.strip_prefix(root).unwrap_or(path),
                None => path,
            };

            #[cfg(windows)]
            let p = if compile_test_index_flags.path_slash_replace {
                use path_slash::PathExt;

                PathBuf::from(p.to_slash().unwrap().into_owned())
            } else {
                p.to_path_buf()
            };

            #[cfg(not(windows))]
            let p = p.to_path_buf();

            p
        }),
        accept_file: Box::new(move |path| {
            if ignore_registry_files.ignore_registry_files && file_is_from_cargo_registry(path) {
                return false;
            }

            true
        }),
        index_size: if compile_test_index_flags.full_index {
            IndexSize::Full
        } else {
            IndexSize::Tiny
        },
    };

    Ok(config)
}

pub fn resolver_for_index_root(
    tmpdir_root: &Path,
    index_root: Option<PathBuf>,
) -> Option<DiscoverIndexPathResolver> {
    index_root.map(|index_root| DiscoverIndexPathResolver::Remap {
        from: tmpdir_root.to_path_buf(),
        to: index_root,
    })
}

pub fn discover_difftests(
    dir: PathBuf,
    index_root: Option<PathBuf>,
    ignore_incompatible: bool,
) -> CargoDifftestsResult<Vec<Difftest>> {
    if !dir.exists() || !dir.is_dir() {
        warn!("Directory {} does not exist", dir.display());
        return Ok(vec![]);
    }

    let resolver = resolver_for_index_root(&dir, index_root);

    let discovered = cargo_difftests::difftest::discover_difftests(
        &dir,
        ignore_incompatible,
        resolver.as_ref(),
    )?;

    Ok(discovered)
}

pub fn display_analysis_result(r: AnalysisResult) {
    let res = match r {
        AnalysisResult::Clean => "clean",
        AnalysisResult::Dirty => "dirty",
    };

    println!("{res}");
}

pub fn cargo_bin_path() -> PathBuf {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let cargo = PathBuf::from(cargo);
    cargo
}

#[derive(Clone, Debug)]
pub struct TestHarness(PathBuf, String);

impl TestHarness {
    pub fn list_tests(&self) -> CargoDifftestsResult<Vec<ListedTest>> {
        let mut tests = vec![];

        let output = std::process::Command::new(&self.0)
            .args(&["--list", "--format=terse"])
            .stdout(std::process::Stdio::piped())
            .env("LLVM_PROFILE_FILE", std::env::temp_dir().join("%m_%p.profraw"))
            .output()?;

        if !output.status.success() {
            bail!("failed to list tests");
        }

        let stdout = String::from_utf8(output.stdout)?;

        for line in stdout.lines() {
            let (trial, kind) = line.split_once(": ").context("invalid test list line")?;
            if kind != "test" {
                continue;
            }
            tests.push(ListedTest(self.clone(), trial.to_owned()));
        }

        Ok(tests)
    }
}

pub struct ListedTest(TestHarness, String);

impl ListedTest {
    pub fn get_harness_name(&self) -> &String {
        &self.0 .1
    }

    pub fn get_harness_path(&self) -> &PathBuf {
        &self.0 .0
    }

    pub fn get_name(&self) -> &String {
        &self.1
    }

    pub fn run_test(
        &self,
        extra: impl FnOnce(&mut std::process::Command) -> &mut std::process::Command,
    ) -> CargoDifftestsResult {
        let output = extra(
            std::process::Command::new(&self.0 .0)
                .args(&["--exact", &self.1, "--nocapture"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped()),
        )
        .output()?;

        if !output.status.success() {
            let stdout = String::from_utf8(output.stdout)?;
            println!("stdout:\n");
            println!("{}", stdout);
            let stderr = String::from_utf8(output.stderr)?;
            error!("stderr:\n");
            error!("{}", stderr);
            bail!("test failed");
        }

        Ok(())
    }

    pub fn run_test_and_collect_profiling_data(&self, difftest_dir: &Path) -> CargoDifftestsResult {
        self.run_test(|cmd| {
            cmd.env("CARGO_DIFFTEST_DIR", &difftest_dir)
                .env("LLVM_PROFILE_FILE", difftest_dir.join("%p_%m.profraw"))
                .env("RUSTC_WORKSPACE_WRAPPER", "rustc-wrapper-difftests")
        })
    }
}

pub fn collect_test_harnesses() -> CargoDifftestsResult<Vec<TestHarness>> {
    let mut harnesses = vec![];

    let mut proc = std::process::Command::new(cargo_bin_path())
        .args(&[
            "test",
            "--no-run",
            "--message-format",
            "json-render-diagnostics",
        ])
        .env("RUSTC_WORKSPACE_WRAPPER", "rustc-wrapper-difftests")
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let stdout = proc.stdout.take().unwrap();

    #[derive(serde::Deserialize, Debug)]
    #[serde(tag = "reason")]
    enum Message {
        #[serde(rename = "compiler-artifact")]
        CompilerArtifact {
            target: TargetSpec,
            executable: Option<PathBuf>,
        },
        #[serde(rename = "build-finished")]
        BuildFinished { success: bool },
        #[serde(rename = "build-script-executed")]
        BuildScriptExecuted {},
    }

    #[derive(serde::Deserialize, Debug)]
    struct TargetSpec {
        kind: Vec<String>,
        name: String,
    }

    let deser = serde_json::StreamDeserializer::new(serde_json::de::IoRead::new(
        std::io::BufReader::with_capacity(2048, stdout),
    ));

    for it in deser {
        let it = it?;

        match it {
            Message::BuildFinished { success } => {
                if !success {
                    bail!("cargo test failed");
                }
            }
            Message::CompilerArtifact { target, executable } => {
                if target.kind.contains(&"test".to_string()) {
                    harnesses.push(TestHarness(executable.unwrap(), target.name));
                }
            }
            Message::BuildScriptExecuted {} => {}
        }
    }

    Ok(harnesses)
}

pub fn get_target_dir() -> CargoDifftestsResult<PathBuf> {
    #[derive(serde::Deserialize)]
    struct Meta {
        target_directory: PathBuf,
    }

    let o = std::process::Command::new(cargo_bin_path())
        .args(&["metadata", "--no-deps", "--format-version", "1"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()?;

    if !o.status.success() {
        let stderr = String::from_utf8(o.stderr)?;
        error!("cargo metadata failed:\n{}", stderr);
        bail!("cargo metadata failed: {}", stderr);
    }

    let meta: Meta = serde_json::from_slice(&o.stdout)?;
    Ok(meta.target_directory)
}

pub fn get_difftests_dir() -> CargoDifftestsResult<PathBuf> {
    match std::env::var_os("CARGO_DIFFTESTS_ROOT") {
        Some(p) => Ok(PathBuf::from(p)),
        None => {
            let target_dir = get_target_dir()?;
            Ok(target_dir.join("tmp").join("difftests"))
        }
    }
}

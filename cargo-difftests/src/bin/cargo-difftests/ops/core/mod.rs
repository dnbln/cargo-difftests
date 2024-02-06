use std::{fs, path::{Path, PathBuf}};

use anyhow::Context;
use cargo_difftests::{analysis::{file_is_from_cargo_registry, AnalysisConfig, AnalysisContext, AnalysisResult}, bin_context::CargoDifftestsContext, difftest::{Difftest, DiscoverIndexPathResolver}, group_difftest::GroupDifftestGroup, index_data::{IndexDataCompilerConfig, IndexSize, TestIndex}};
use log::warn;

use crate::{cli_core::{AnalysisIndex, AnalysisIndexStrategy, CompileTestIndexFlags, DirtyAlgorithm, ExportProfdataConfigFlags, FlattenFilesTarget, IgnoreRegistryFilesFlag}, CargoDifftestsResult};

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

pub fn analyze_single_group(
    ctxt: &CargoDifftestsContext,
    group: &mut GroupDifftestGroup,
    force: bool,
    algo: DirtyAlgorithm,
    commit: Option<git2::Oid>,
    analysis_index: &AnalysisIndex,
    resolver: Option<&DiscoverIndexPathResolver>,
    ignore_registry_files: IgnoreRegistryFilesFlag,
) -> CargoDifftestsResult<AnalysisResult> {
    let mut analysis_cx = match analysis_index.index_strategy {
        AnalysisIndexStrategy::Never => {
            group.merge_profraws(force)?;

            group.start_analysis(ignore_registry_files.ignore_registry_files)?
        }
        AnalysisIndexStrategy::Always => {
            'l: {
                if group.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest_group(group)?;
                }

                group.merge_profraws(force)?;

                let config = compile_test_index_config(
                    analysis_index.compile_test_index_flags.clone(),
                    ignore_registry_files,
                )?;

                let test_index_data = group.compile_test_index_data(config)?;

                if let Some(p) = resolver.and_then(|r| r.resolve(group.dir())) {
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
                if group.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest_group(group)?;
                }

                group.merge_profraws(force)?;

                let config = compile_test_index_config(
                    analysis_index.compile_test_index_flags.clone(),
                    ignore_registry_files,
                )?;

                let test_index_data = group.compile_test_index_data(config)?;

                if let Some(p) = resolver.and_then(|r| r.resolve(group.dir())) {
                    let parent = p.parent().unwrap();
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                    test_index_data.write_to_file(&p)?;

                    group.clean()?;
                }

                AnalysisContext::from_index(test_index_data)
            }
        }
        AnalysisIndexStrategy::IfAvailable => {
            'l: {
                if group.has_index() {
                    // if we already have the index built, use it
                    break 'l AnalysisContext::with_index_from_difftest_group(group)?;
                }

                group.merge_profraws(force)?;

                group.start_analysis(ignore_registry_files.ignore_registry_files)?
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
        remove_bin_path: compile_test_index_flags.remove_bin_path,
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

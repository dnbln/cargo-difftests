use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::SystemTime,
};

use cargo_difftests_core::CoreTestDesc;
use log::{debug, info, warn};

use crate::{
    analysis::AnalysisContext,
    analysis_data,
    difftest::{DiscoverIndexPathResolver, ProfDataExportable, ProfrawsMergeable},
    index_data::{IndexDataCompilerConfig, TestIndex},
    DifftestsError, DifftestsResult,
};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GroupDifftest {
    dir: PathBuf,
    self_profraw: PathBuf,
    other_profraws: Vec<PathBuf>,
    self_json: PathBuf,

    bin_path: PathBuf,
}

impl GroupDifftest {
    pub fn load_test_desc(&self) -> DifftestsResult<CoreTestDesc> {
        let desc = std::fs::read_to_string(&self.self_json)?;
        Ok(serde_json::from_str(&desc)
            .map_err(|e| DifftestsError::Json(e, Some(self.self_json.clone())))?)
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GroupDifftestGroup {
    dir: PathBuf,
    difftest_list: Vec<GroupDifftest>,
    main_group_binary: PathBuf,
    other_bin_paths: Vec<PathBuf>,

    profdata_file: Option<PathBuf>,
    index_data: Option<PathBuf>,

    cleaned: bool,
    mtime: SystemTime,
}

impl GroupDifftestGroup {
    pub fn dir(&self) -> &Path {
        self.dir.as_path()
    }

    pub fn merge_profraws(&mut self, force: bool) -> DifftestsResult<()> {
        if self.cleaned {
            return Err(DifftestsError::DifftestCleaned);
        }

        if self.profdata_file.is_some() && !force {
            return Ok(());
        }

        crate::difftest::merge_profraws(self)?;

        self.profdata_file = Some(self.out_profdata_path());

        Ok(())
    }

    pub fn export_profdata(
        &self,
        ignore_registry_files: bool,
    ) -> DifftestsResult<analysis_data::CoverageData> {
        if self.cleaned {
            return Err(DifftestsError::DifftestCleaned);
        }

        assert!(self.profdata_file.is_some());

        let result = crate::difftest::export_profdata_file(
            self,
            ignore_registry_files,
            crate::difftest::ExportProfdataAction::Read,
        )?;

        Ok(result.coverage_data())
    }

    pub fn start_analysis(&self, ignore_registry_files: bool) -> DifftestsResult<AnalysisContext> {
        info!("Starting analysis...");
        debug!(
            "Reading exported profdata file from {:?}...",
            self.profdata_path()
        );
        let profdata = self.export_profdata(ignore_registry_files)?;
        debug!(
            "Done reading exported profdata file from {:?}.",
            self.profdata_path()
        );

        Ok(AnalysisContext::new_from_difftest_group(self, profdata))
    }

    pub fn mtime(&self) -> SystemTime {
        self.mtime
    }

    pub fn has_index(&self) -> bool {
        self.index_data.is_some()
    }

    pub fn read_index_data(&self) -> DifftestsResult<Option<TestIndex>> {
        let Some(p) = self.index_data.as_ref() else {
            return Ok(None);
        };
        TestIndex::read_from_file(p).map(Some)
    }

    pub fn compile_test_index_data(
        &mut self,
        index_data_compiler_config: IndexDataCompilerConfig,
    ) -> DifftestsResult<TestIndex> {
        info!("Compiling test index data...");

        let profdata = self.export_profdata(index_data_compiler_config.ignore_registry_files)?;
        let test_index_data = TestIndex::index_group(self, profdata, index_data_compiler_config)?;

        info!("Done compiling test index data.");
        Ok(test_index_data)
    }

    pub fn load_test_descriptions(&self) -> DifftestsResult<Vec<CoreTestDesc>> {
        let mut test_desc = Vec::new();

        for difftest in &self.difftest_list {
            let desc = difftest.load_test_desc()?;
            test_desc.push(desc);
        }

        Ok(test_desc)
    }

    pub fn clean(&mut self) -> DifftestsResult<()> {
        if self.cleaned {
            return Ok(());
        }

        info!("Cleaning group...");

        fn clean_file(path: &mut Option<PathBuf>) -> DifftestsResult<()> {
            if let Some(path) = path {
                std::fs::remove_file(path)?;
            }
            *path = None;
            Ok(())
        }

        for difftest in &mut self.difftest_list {
            std::fs::write(&difftest.self_profraw, "")?;
            for profraw in difftest.other_profraws.drain(..) {
                std::fs::remove_file(profraw)?;
            }
        }

        clean_file(&mut self.profdata_file)?;

        self.cleaned = true;

        info!("Done cleaning group.");

        Ok(())
    }
}

impl ProfrawsMergeable for GroupDifftestGroup {
    fn list_profraws(&self) -> impl Iterator<Item = &Path> {
        self.difftest_list
            .iter()
            .flat_map(|it| std::iter::once(&it.self_profraw).chain(it.other_profraws.iter()))
            .map(PathBuf::as_path)
    }

    fn out_profdata_path(&self) -> PathBuf {
        self.dir.join(GROUP_PROFDATA_FILENAME)
    }
}

const GROUP_PROFDATA_FILENAME: &str = "group.profdata";
const CLEANED_FILENAME: &str = "group_cleaned";

pub fn index_group(
    dir: PathBuf,
    ignore_incompatible: bool,
    other_bin_paths: Vec<PathBuf>,
    index_resolver: Option<&DiscoverIndexPathResolver>,
) -> DifftestsResult<GroupDifftestGroup> {
    // we will need the time of the oldest test run, as well as the newest file change, to compare them.
    struct MinSystemTime(SystemTime);

    impl Default for MinSystemTime {
        fn default() -> Self {
            Self(SystemTime::now())
        }
    }

    impl std::iter::FromIterator<SystemTime> for MinSystemTime {
        fn from_iter<T: IntoIterator<Item = SystemTime>>(iter: T) -> Self {
            let mut min = Self::default();
            for it in iter {
                if it < min.0 {
                    min.0 = it;
                }
            }
            min
        }
    }

    impl Extend<SystemTime> for MinSystemTime {
        fn extend<T: IntoIterator<Item = SystemTime>>(&mut self, iter: T) {
            let new_min = iter.into_iter().collect::<Self>();
            if new_min.0 < self.0 {
                self.0 = new_min.0;
            }
        }
    }

    let (difftests, MinSystemTime(mtime)) =
        crate::difftest::discover_difftests(&dir, ignore_incompatible, index_resolver)?
            .into_iter()
            .filter(|it| it.profdata_file.is_none() && !it.was_cleaned())
            .map(|it| {
                let bin_path = it.load_test_desc()?.bin_path;
                let mtime = it.self_json_mtime()?;
                Ok((
                    GroupDifftest {
                        dir: it.dir,
                        self_profraw: it.self_profraw,
                        other_profraws: it.other_profraws,
                        self_json: it.self_json,
                        bin_path,
                    },
                    mtime,
                ))
            })
            .collect::<DifftestsResult<Vec<_>>>()?
            .into_iter()
            .unzip::<GroupDifftest, SystemTime, Vec<_>, MinSystemTime>();

    if difftests.is_empty() {
        return Err(crate::DifftestsError::EmptyGroup(dir));
    }

    let main_group_binary = difftests
        .iter()
        .map(|it| &it.bin_path)
        .collect::<HashSet<_>>();
    if main_group_binary.len() > 1 {
        return Err(crate::DifftestsError::MultipleMainGroupBinaries(
            dir,
            main_group_binary.into_iter().cloned().collect(),
        ));
    }

    let main_group_binary = main_group_binary.into_iter().next().unwrap().clone();

    let profdata_file = dir.join(GROUP_PROFDATA_FILENAME);
    let profdata_file = if profdata_file.exists() {
        let profdata_mtime = std::fs::metadata(&profdata_file)?.modified()?;
        if profdata_mtime < mtime {
            warn!(
                "Profdata file {:?} is older than the newest test run ({:?} < {:?}).",
                profdata_file, profdata_mtime, mtime
            );
            info!("You might want to use the --force flag.");
        }

        Some(profdata_file)
    } else {
        None
    };

    let index_data = match index_resolver {
        Some(index_resolver) => match index_resolver.resolve(&dir) {
            Some(p) if p.exists() => {
                let index_mtime = std::fs::metadata(&p)?.modified()?;
                if index_mtime < mtime {
                    warn!(
                        "Index file {:?} is older than the newest test run ({:?} < {:?}).",
                        p, index_mtime, mtime
                    );
                    info!("You might want to use the --force flag.");
                }

                Some(p)
            }
            _ => None,
        },
        None => None,
    };

    let cleaned = dir.join(CLEANED_FILENAME).exists();

    Ok(GroupDifftestGroup {
        dir,
        main_group_binary,
        difftest_list: difftests,
        other_bin_paths,
        profdata_file,
        index_data,
        cleaned,
        mtime,
    })
}

impl ProfDataExportable for GroupDifftestGroup {
    fn profdata_path(&self) -> &Path {
        self.profdata_file
            .as_ref()
            .expect("Missing profdata")
            .as_path()
    }

    fn main_bin_path(&self) -> DifftestsResult<PathBuf> {
        Ok(self.main_group_binary.clone())
    }

    fn other_bins(&self) -> impl Iterator<Item = &Path> {
        self.other_bin_paths.iter().map(PathBuf::as_path)
    }
}

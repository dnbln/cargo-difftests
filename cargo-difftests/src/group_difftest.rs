/*
 *        Copyright (c) 2023-2024 Dinu Blanovschi
 *
 *    Licensed under the Apache License, Version 2.0 (the "License");
 *    you may not use this file except in compliance with the License.
 *    You may obtain a copy of the License at
 *
 *        https://www.apache.org/licenses/LICENSE-2.0
 *
 *    Unless required by applicable law or agreed to in writing, software
 *    distributed under the License is distributed on an "AS IS" BASIS,
 *    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *    See the License for the specific language governing permissions and
 *    limitations under the License.
 */

use std::{
    path::{Path, PathBuf},
    time::SystemTime, ffi::OsStr,
};

use cargo_difftests_core::{CoreTestDesc, CoreGroupDesc};
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
    self_json: PathBuf,
    self_profraw: PathBuf,
    other_profraws: Vec<PathBuf>,
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

    pub fn load_self_json(&self) -> DifftestsResult<CoreGroupDesc> {
        let desc = std::fs::read_to_string(&self.self_json)?;
        Ok(serde_json::from_str(&desc)
            .map_err(|e| DifftestsError::Json(e, Some(self.self_json.clone())))?)
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

        clean_file(&mut self.profdata_file)?;

        self.cleaned = true;

        info!("Done cleaning group.");

        Ok(())
    }
}

impl ProfrawsMergeable for GroupDifftestGroup {
    fn list_profraws(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(&self.self_profraw)
            .chain(self.other_profraws.iter())
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
    other_bin_paths: Vec<PathBuf>,
    index_resolver: Option<&DiscoverIndexPathResolver>,
) -> DifftestsResult<GroupDifftestGroup> {
    let mtime = dir.join(cargo_difftests_core::CARGO_DIFFTESTS_GROUP_FIRST_TEST_RUN).metadata()?.modified()?;

    let self_json = dir.join(cargo_difftests_core::CARGO_DIFFTESTS_GROUP_SELF_JSON_FILENAME);

    if !self_json.exists() {
        return Err(DifftestsError::SelfJsonDoesNotExist(self_json));
    }

    let self_profraw = dir.join(cargo_difftests_core::CARGO_DIFFTESTS_SELF_PROFILE_FILENAME);

    if !self_profraw.exists() {
        return Err(DifftestsError::SelfProfrawDoesNotExist(self_profraw));
    }

    let cargo_difftests_version = dir.join(cargo_difftests_core::CARGO_DIFFTESTS_VERSION_FILENAME);

    if !cargo_difftests_version.exists() {
        return Err(DifftestsError::CargoDifftestsVersionDoesNotExist(
            cargo_difftests_version,
        ));
    }

    let version = std::fs::read_to_string(&cargo_difftests_version)?;

    if version != env!("CARGO_PKG_VERSION") {
        return Err(DifftestsError::CargoDifftestsVersionMismatch(
            version,
            env!("CARGO_PKG_VERSION").to_owned(),
        ));
    }

    let mut other_profraws = Vec::new();

    let mut profdata_file = None;

    let mut cleaned = false;

    for e in dir.read_dir()? {
        let e = e?;
        let p = e.path();

        if !p.is_file() {
            continue;
        }

        let file_name = p.file_name();
        let ext = p.extension();

        if ext == Some(OsStr::new("profraw"))
            && file_name
                != Some(OsStr::new(
                    cargo_difftests_core::CARGO_DIFFTESTS_SELF_PROFILE_FILENAME,
                ))
        {
            other_profraws.push(p);
            continue;
        }

        if ext == Some(OsStr::new("profdata")) {
            if profdata_file.is_none() {
                profdata_file = Some(p);
            } else {
                warn!(
                    "multiple profdata files found in difftest directory: {}",
                    dir.display()
                );
                warn!("ignoring: {}", p.display());
            }
            continue;
        }

        if file_name == Some(OsStr::new(CLEANED_FILENAME)) {
            cleaned = true;
        }
    }


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

    let profdata_file = if let Some(profdata_file) = profdata_file {
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

    Ok(GroupDifftestGroup {
        dir,
        self_json,
        self_profraw,
        other_profraws,
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
        self.load_self_json().map(|desc| desc.bin_path)
    }

    fn other_bins(&self) -> impl Iterator<Item = &Path> {
        self.other_bin_paths.iter().map(PathBuf::as_path)
    }
}

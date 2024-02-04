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

#![cfg(any(cargo_difftests, docsrs))]

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use cargo_difftests_core::CoreTestDesc;

mod llvm_profiling;

/// A description of a test.
///
/// This is used to identify the test, and the binary from which it came from.
/// `cargo difftests` only uses the `bin_path`, all the other fields can
/// have any values you'd like to give them.
pub struct TestDesc<T: serde::Serialize> {
    /// The binary path.
    pub bin_path: PathBuf,
    /// Any other fields to help identify the test.
    pub extra: T,
}

struct SelfProfileWriter {
    llvm_profile_self_file: PathBuf,
}

impl SelfProfileWriter {
    fn do_write_to_file(file: &std::path::Path) {
        unsafe {
            #[allow(temporary_cstring_as_ptr)]
            llvm_profiling::__llvm_profile_set_filename(
                std::ffi::CString::new(file.to_str().unwrap())
                    .unwrap()
                    .as_ptr(),
            );
            let r = llvm_profiling::__llvm_profile_write_file();
            assert_eq!(r, 0);
        }
    }
}

impl Drop for SelfProfileWriter {
    fn drop(&mut self) {
        Self::do_write_to_file(&self.llvm_profile_self_file);
    }
}

#[cfg(feature = "compile-index-and-clean")]
pub mod compile_index_and_clean_config;

enum DifftestsEnvInner {
    Test {
        #[allow(dead_code)]
        self_profile_drop_writer: SelfProfileWriter,

        #[cfg(all(
            feature = "enforce-single-running-test",
            not(feature = "parallel-groups")
        ))]
        _t_lock: std::sync::MutexGuard<'static, ()>,

        #[cfg(feature = "compile-index-and-clean")]
        compile_index_and_clean: Option<compile_index_and_clean_config::RunOnDrop>,

        #[cfg(feature = "compile-index-and-clean")]
        self_dir: PathBuf,
    },
    #[cfg(feature = "groups")]
    Group(#[allow(dead_code)] groups::GroupDifftestsEnv),
}

#[cfg(feature = "parallel-groups")]
impl Drop for DifftestsEnvInner {
    fn drop(&mut self) {
        match self {
            DifftestsEnvInner::Test { .. } => {
                let mut _l = groups::wr_test_group_dec();
                *_l = groups::State::None;
                groups::crs_notify();
            }
            DifftestsEnvInner::Group(_) => {}
        }
    }
}

/// The difftests environment.
pub struct DifftestsEnv {
    llvm_profile_file_name: OsString,
    llvm_profile_file_value: OsString,

    #[allow(dead_code)]
    difftests_env_inner: DifftestsEnvInner,
}

#[cfg(all(
    feature = "enforce-single-running-test",
    not(feature = "parallel-groups")
))]
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = LOCK.get_or_init(|| Mutex::new(()));
    lock.lock().unwrap()
}

impl DifftestsEnv {
    /// Returns an iterator over the environment variables that should be set
    /// for child processes.
    pub fn env_for_children(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        std::iter::once((
            self.llvm_profile_file_name.as_os_str(),
            self.llvm_profile_file_value.as_os_str(),
        ))
    }

    #[cfg(feature = "compile-index-and-clean")]
    pub fn and_compile_index_and_clean_on_exit(
        mut self,
        f: impl FnOnce(
            compile_index_and_clean_config::CompileIndexAndCleanConfigBuilder,
        ) -> compile_index_and_clean_config::CompileIndexAndCleanConfigBuilder,
    ) -> Self {
        match &mut self.difftests_env_inner {
            DifftestsEnvInner::Test {
                compile_index_and_clean,
                self_dir,
                ..
            } => {
                let c = compile_index_and_clean_config::CompileIndexAndCleanConfigBuilder::new(
                    self_dir.clone(),
                );
                let c = f(c);
                *compile_index_and_clean = Some(compile_index_and_clean_config::RunOnDrop::new(c.build()));
            }
            DifftestsEnvInner::Group { .. } => {
                panic!(
                    "and_compile_index_on_exit can only be called in a non-group test environment"
                );
            }
        }

        self
    }
}

/// Initializes the difftests environment.
pub fn init<T: serde::Serialize>(
    desc: TestDesc<T>,
    tmpdir: &Path,
) -> std::io::Result<DifftestsEnv> {
    #[cfg(all(
        feature = "enforce-single-running-test",
        not(feature = "parallel-groups")
    ))]
    let _t_lock = test_lock();

    #[cfg(feature = "parallel-groups")]
    groups::wr_test_group_inc(None);

    if tmpdir.exists() {
        std::fs::remove_dir_all(tmpdir)?;
    }
    std::fs::create_dir_all(tmpdir)?;

    let self_profile_file =
        tmpdir.join(cargo_difftests_core::CARGO_DIFFTESTS_SELF_PROFILE_FILENAME);

    std::fs::write(&self_profile_file, "")?;

    let self_info_path = tmpdir.join(cargo_difftests_core::CARGO_DIFFTESTS_SELF_JSON_FILENAME);

    let core_test_desc = CoreTestDesc {
        bin_path: desc.bin_path,
        extra: serde_json::to_value(&desc.extra).unwrap(),
    };

    let self_info = serde_json::to_string(&core_test_desc).unwrap();

    std::fs::write(self_info_path, self_info)?;

    std::fs::write(
        tmpdir.join(cargo_difftests_core::CARGO_DIFFTESTS_VERSION_FILENAME),
        env!("CARGO_PKG_VERSION"),
    )?;

    // and for children
    let profraw_path =
        tmpdir.join(cargo_difftests_core::CARGO_DIFFTESTS_OTHER_PROFILE_FILENAME_TEMPLATE);

    let r = Ok(DifftestsEnv {
        llvm_profile_file_name: "LLVM_PROFILE_FILE".into(),
        llvm_profile_file_value: profraw_path.into(),
        difftests_env_inner: DifftestsEnvInner::Test {
            self_profile_drop_writer: SelfProfileWriter {
                llvm_profile_self_file: self_profile_file,
            },

            #[cfg(all(
                feature = "enforce-single-running-test",
                not(feature = "parallel-groups")
            ))]
            _t_lock,

            #[cfg(feature = "compile-index-and-clean")]
            compile_index_and_clean: None,

            #[cfg(feature = "compile-index-and-clean")]
            self_dir: tmpdir.to_path_buf(),
        },
    });

    unsafe {
        llvm_profiling::__llvm_profile_reset_counters();
    }

    r
}

pub mod groups;

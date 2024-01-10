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

#[cfg(cargo_difftests)]
extern "C" {
    pub static __llvm_profile_runtime: i32;
    fn __llvm_profile_set_filename(filename: *const libc::c_char);
    fn __llvm_profile_write_file() -> libc::c_int;
    fn __llvm_profile_reset_counters();
}

// put dummies for docs.rs
#[cfg(all(not(cargo_difftests), docsrs))]
unsafe fn __llvm_profile_set_filename(_: *const libc::c_char) {}

#[cfg(all(not(cargo_difftests), docsrs))]
unsafe fn __llvm_profile_write_file() -> libc::c_int {
    0
}

#[cfg(all(not(cargo_difftests), docsrs))]
unsafe fn __llvm_profile_reset_counters() {}

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

/// The difftests environment.
pub struct DifftestsEnv {
    llvm_profile_file_name: OsString,
    llvm_profile_file_value: OsString,

    llvm_profile_self_file: PathBuf,

    #[cfg(feature = "enforce-single-running-test")]
    _t_lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(feature = "enforce-single-running-test")]
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
}

impl Drop for DifftestsEnv {
    fn drop(&mut self) {
        unsafe {
            #[allow(temporary_cstring_as_ptr)]
            __llvm_profile_set_filename(
                std::ffi::CString::new(self.llvm_profile_self_file.to_str().unwrap())
                    .unwrap()
                    .as_ptr(),
            );
            let r = __llvm_profile_write_file();
            assert_eq!(r, 0);
        }
    }
}

/// Initializes the difftests environment.
pub fn init<T: serde::Serialize>(
    desc: TestDesc<T>,
    tmpdir: &Path,
) -> std::io::Result<DifftestsEnv> {
    #[cfg(feature = "enforce-single-running-test")]
    let _t_lock = test_lock();

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
        llvm_profile_self_file: self_profile_file,

        #[cfg(feature = "enforce-single-running-test")]
        _t_lock,
    });

    unsafe {
        __llvm_profile_reset_counters();
    }

    r
}

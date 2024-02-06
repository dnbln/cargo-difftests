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

#[cfg(cargo_difftests)]
extern "C" {
    pub static __llvm_profile_runtime: i32;
    pub fn __llvm_profile_set_filename(filename: *const libc::c_char);
    pub fn __llvm_profile_write_file() -> libc::c_int;
    pub fn __llvm_profile_reset_counters();
}

// put dummies for docs.rs
#[cfg(all(not(cargo_difftests), docsrs))]
pub unsafe fn __llvm_profile_set_filename(_: *const libc::c_char) {}

#[cfg(all(not(cargo_difftests), docsrs))]
pub unsafe fn __llvm_profile_write_file() -> libc::c_int {
    0
}

#[cfg(all(not(cargo_difftests), docsrs))]
pub unsafe fn __llvm_profile_reset_counters() {}

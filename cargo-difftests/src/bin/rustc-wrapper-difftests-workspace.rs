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

#![feature(exit_status_error)]
#![feature(array_windows)]

use std::process::ExitCode;

#[path ="rustc_wrapper_impl/mod.rs"]
mod rustc_wrapper_impl;

fn main() -> std::io::Result<ExitCode> {
    rustc_wrapper_impl::rustc_wrapper_impl(true)
}
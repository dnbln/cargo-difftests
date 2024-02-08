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

fn main() -> std::io::Result<ExitCode> {
    let mut args = std::env::args().skip(1);
    let rustc = args.next().unwrap();
    let mut remaining = args.collect::<Vec<_>>();

    if !remaining
        .array_windows::<2>()
        .any(|[a, b]| a == "-C" && b == "instrument-coverage")
    {
        remaining.push("-C".to_owned());
        remaining.push("instrument-coverage".to_owned());
    }

    let mut cmd = std::process::Command::new(rustc);
    cmd.args(&remaining);
    match cmd.status()?.exit_ok() {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(e) => {
            eprintln!("error: {}", e);
            Ok(ExitCode::FAILURE)
        }
    }
}

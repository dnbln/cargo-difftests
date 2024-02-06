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

use cargo_difftests::bin_context::CargoDifftestsContext;
use clap::Parser;
use ops::CargoApp;
use prodash::render::line;

mod cli_core;
mod ops;

pub type CargoDifftestsResult<T = ()> = anyhow::Result<T>;

fn main_impl() -> CargoDifftestsResult {
    pretty_env_logger::formatted_builder()
        .parse_env("CARGO_DIFFTESTS_LOG")
        .init();

    let (ctxt, tree) = CargoDifftestsContext::new();

    let handle = line::render(
        std::io::stderr(),
        tree,
        line::Options {
            frames_per_second: 20.0,
            ..Default::default()
        }
        .auto_configure(line::StreamKind::Stderr),
    );

    let CargoApp::Difftests { app } = CargoApp::parse();

    app.run(&ctxt)?;

    handle.shutdown_and_wait();

    Ok(())
}

fn main() {
    if let Err(e) = main_impl() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

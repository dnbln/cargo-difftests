#![feature(exit_status_error)]
#![feature(array_windows)]

use std::process::ExitCode;

#[path ="rustc_wrapper_impl/mod.rs"]
mod rustc_wrapper_impl;

fn main() -> std::io::Result<ExitCode> {
    rustc_wrapper_impl::rustc_wrapper_impl(false)
}
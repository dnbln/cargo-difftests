use std::process::ExitCode;

fn main() -> std::io::Result<ExitCode> {
    rustc_wrapper_difftests::rustc_wrapper_impl()
}

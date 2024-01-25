use std::process::ExitCode;

fn is_difftests_profile(remaining: &[String]) -> bool {
    remaining
        .array_windows::<2>()
        .any(|[a, b]| a == "--cfg" && b == "cargo_difftests")
}

pub fn rustc_wrapper_impl(is_workspace_member: bool) -> std::io::Result<ExitCode> {
    let mut args = std::env::args().skip(1);
    let rustc = args.next().unwrap();
    let mut remaining = args.collect::<Vec<_>>();

    if is_difftests_profile(&remaining) {
        if is_workspace_member
            && !remaining
                .array_windows::<2>()
                .any(|[a, b]| a == "-C" && b == "instrument-coverage")
        {
            remaining.push("-C".to_owned());
            remaining.push("instrument-coverage".to_owned());
        }
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

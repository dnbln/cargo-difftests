# Spawning subprocesses

You can go ahead and spawn subprocesses normally; if their
coverage information should be ignored. If you however want them
to be tracked, you should pass them some environment variables.

## Environment variables

You can pass the required environment variables to the subprocess
like this:

```Rust
#[test]
fn test_f() {
    let env: DifftestsEnv = setup_difftests("test_f");
    let mut cmd = std::process::Command::new("exe");
    // the DifftestsEnv::env_for_children()
    // method gives us the required environment variables
    #[cfg(cargo_difftests)]
    cmd.envs(env.env_for_children());
    cmd.spawn().unwrap().wait().unwrap();
}
```

The instrumented binaries must have had coverage enabled for this to work.

Unlike with [multithreading](multithreading.md), the subprocesses can run
even after the test has finished, and they will still be tracked. They
only have to finish before the analysis. However, leaky tests are still
a problem, and should be avoided.

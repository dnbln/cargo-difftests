# Manual setup

This tutorial goes over how to set up `cargo-difftests` for an already existing project.

In this tutorial, you will learn how to:

* Set up a `difftests` cargo profile for the project, to be able to run the tests.
* Patch the tests, so that they use the `cargo-difftests-testclient` crate to
  set up the profiling runtime such that `cargo-difftests` will be able to analyze
  the outputs later.

## Before you start

Make sure that:

- You understand what you're getting yourself into. Have you read the [overview](Overview.md)?
- You have [nightly rust](https://rust-lang.github.io/rustup/concepts/channels.html) installed.
- You have [`cargo-binutils`](https://github.com/rust-embedded/cargo-binutils) installed.
- You also have `cargo-difftests` installed:

```Bash
cargo install cargo-difftests
```

- `cargo-difftests` supports 2 modes of collecting profiling data: For all crates, or for workspace crates only. In
  practice, both of those work, but big projects that tend to have a lot of dependencies would generate substantially
  larger profiles, which in turn makes their analysis much slower. So in general it is recommended to
  only `-C instrument-coverage` workspace crates.

<tabs group="coverage-area">
<tab title="Coverage for all the crates" group-key="all">
</tab>
<tab title="Workspace-only coverage" group-key="workspace-only">
In the case of workspace-only coverage, there is an additional package to install:

```Bash
cargo install rustc-wrapper-difftests
```

</tab>
</tabs>

## Creating the `difftests` profile

This section will go over how to create
the `difftests` [cargo profile](https://doc.rust-lang.org/cargo/reference/profiles.html).

First, if your project doesn't have a <path>.cargo/config.toml</path> file yet, go ahead and create it.
Once it is there, you need to add those few lines:

<tabs group="coverage-area">
<tab title="Coverage for all the crates" group-key="all">

```Toml
# .cargo/config.toml

[profile.difftests]
inherits = "dev"
rustflags = [
  "-C", "instrument-coverage",
  "--cfg", "cargo_difftests",
]

[unstable]
profile-rustflags = true

```

</tab>
<tab title="Workspace-only coverage" group-key="workspace-only">

```Toml
# .cargo/config.toml

[profile.difftests]
inherits = "dev"
rustflags = [
  "--cfg", "cargo_difftests",
]

[build]
rustc-wrapper = "rustc-wrapper-difftests"
rustc-workspace-wrapper = "rustc-wrapper-difftests-workspace"

[unstable]
profile-rustflags = true
```

</tab>
</tabs>

We now have a `difftests` profile. You can use it to run your tests:

```Bash
cargo test --profile difftests
```

And it would generate a few `.profraw` files. But they are not in the correct place yet, and we're still lacking the
metadata that would allow `cargo-difftests` to work. The next section goes over how to set up the metadata and the
folder
structure that `cargo-difftests` knows how to work with.

## Setting up `cargo-difftests-testclient` {id="how-to-write-tests-for-cargo-difftests"}

```Toml
[dependencies]
cargo-difftests-testclient = "0.5.0"
serde = { version = "1", features = ["derive"] }
```

Now this is where the fun begins. There's really only one function you should know about, and that is:
[`cargo_difftests_testclient::init`].

> Profiling data collected before this function was called will be ignored. This allows multiple tests to be run in the
> same process and still provide accurate data, so "it's not a bug, it's a feature."
>
> As such, it would be best if this function was called right upon entry in the test.
> {style="warning"}

The value it returns is quite important, here's what you have to keep in mind while writing tests:

> It's a RAII guard which will write the profiling data when `drop()`-ed. Dropping it right after the call to `init()`
> will write out the corresponding profiling data, which would not include most of (or any of) the files that contained
> code that was executed in the test, which would in turn lead to inaccurate results.
>
{style="warning"}

> When spawning children that should also be profiled, you will need to pass
> some special environment variables to them, like this:
>
> ```Rust
> let difftests_env = cargo_difftests_testclient::init(...);
> 
> // ...
> let mut cmd = Command::new("...");
> cmd.envs(difftests_env.env_for_children());
> ```
{style="note"}

### `init` function parameters

The parameters to the `init` function are as follows:

The first parameter is a "description" of the test. It includes the path of the test binary, as it is needed by
`cargo-difftests` to work, but other than that, it has only one generic field: `extra`.

#### What to put in the extra field? {id="test-desc-extra-field"}

Generally, you would want the test name here, as well as anything else you might need to identify the test.
`cargo-difftests` mostly puts the responsibility of rerunning dirty tests to the developers, given how
certain test pipelines of certain projects could get incredibly complex. In most cases however, knowing
only the test name is enough, so we should make a type like:

```Rust
#[derive(Clone, serde::Serialize)]
struct ExtraArgs {
    pkg_name: String,
    crate_name: String,
    test_name: String,
}
```

#### The temporary directory {id="what-to-pass-for-temp-dir"}

The `init` function takes a `&Path` as its second parameter. This is the directory where it should be free to store all
the files `cargo-difftests` will later use for analysis. This should be passed to various `cargo difftests` subcommands
as the `--dir` option.

It generally is enough to just use something along the lines of the following, but you can change this
and `cargo-difftests` will still work:

```Rust
let tmpdir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("cargo-difftests")
        .join(test_name);
```

### Calling `init`

Putting it all together, it should look something like this.
We can `#[cfg]` code based on the `--cfg cargo_difftests` that we added above.

```Rust
#[cfg(cargo_difftests)]
type DifftestsEnv = cargo_difftests_testclient::DifftestsEnv;

#[cfg(not(cargo_difftests))]
type DifftestsEnv = ();

#[cfg(cargo_difftests)]
#[derive(serde::Serialize, Clone)]
struct ExtraArgs {
    pkg_name: String,
    crate_name: String,
    test_name: String,
}

#[must_use]
fn setup_difftests(test_name: &str) -> DifftestsEnv {
    #[cfg(cargo_difftests)]
    {
        let tmpdir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("cargo-difftests")
            .join(test_name);
        let difftests_env = cargo_difftests_testclient::init(
            cargo_difftests_testclient::TestDesc::<ExtraArgs> {
                bin_path: std::env::current_exe().unwrap(),
                extra: ExtraArgs {
                    pkg_name: env!("CARGO_PKG_NAME").to_string(),
                    crate_name: env!("CARGO_CRATE_NAME").to_string(),
                    test_name: test_name.to_string(),
                },
            },
            &tmpdir,
        )
        .unwrap()
    }
}
```

```Rust
#[test]
fn test_fn() {
    let _env = setup_difftests("test_fn"); // _env is a RAII-style guard
    assert_eq!(1 + 1, 2);
    // now dropping _env would write the profiling data out to the file where cargo-difftests expects it to be.
}
```

{collapsible="true" collapsed-title="An example test"}

## Invoking `cargo-difftests`

Let's first add a function to <path>src/lib.rs</path> which we could test.

```Rust
// src/lib.rs
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

{collapsible="true" collapsed-title="lib.rs"}

And a test to use that to <path>tests/tests.rs</path>.

```Rust
// tests/tests.rs
// setup_difftests defined above

#[test]
fn test_add() {
    let _env = setup_difftests("test_add");
    assert_eq!(crate_name::add(1, 1), 2);
}
```

{collapsible="true" collapsed-title="tests/tests.rs"}

```Bash
> cargo test --profile difftests
> cargo difftests analyze --dir target/tmp/cargo-difftests/test_add
clean
> touch src/lib.rs
> cargo difftests analyze --dir target/tmp/cargo-difftests/test_add
dirty
> cargo test --profile difftests test_add
> cargo difftests analyze --dir target/tmp/cargo-difftests/test_add
clean
> touch tests/tests.rs
> cargo difftests analyze --dir target/tmp/cargo-difftests/test_add
dirty
> cargo test --profile difftests test_add
> cargo difftests analyze --dir target/tmp/cargo-difftests/test_add
clean
```

`dirty` means that the test should be rerun, `clean` means that none of the files that contained code executed by the
test changed, so most likely rerunning the test would not change anything.

## What you've learned {id="what-learned"}

In this tutorial you've learned how to set up your project to use `cargo-difftests`, as well as how to use it find if a
test should be rerun or not.

### Where to go from here

[//]: # (<seealso>)
<!--Give some related links to how-to articles-->

- [How to use the `analyze-all` subcommand to automatically find all tests and analyze them.](Use-with-analyze-all.md)

[//]: # (</seealso>)

[`cargo_difftests_testclient::init`]: https://docs.rs/cargo-difftests-testclient/latest/cargo_difftests_testclient/fn.init.html

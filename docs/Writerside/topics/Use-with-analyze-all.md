# How to use with the analyze-all command

Right now we are only checking tests individually, but by using this command
you could analyze all the tests in one go. Although that sounds really nice,
it's output was only ever intended to be read by machines: what it actually
outputs is a huge JSON string. This section will go over how to analyze that,
and / or get `cargo-difftests` to actually rerun the tests.

> Under construction.
> {style="warning"}

The actual type of the JSON is an array of [AnalyzeAllSingleTestGroup],
if you would like to parse that.

If you only want to rerun the tests, then maybe this section is for you.

## Automatic test-rerunning

`cargo-difftests analyze-all` accepts an `--action` option, which can
be either `print` (default), `assert-clean` or `rerun-dirty`.

### `--action=print`

As the name implies, this only prints out the JSON corresponding to
all the analysis results, and leaves it up to the calling process to
do something with that.

### `--action=assert-clean`

This action analyzes all the difftest directories that were
discovered, and if any of them is dirty, then it errors (and
exits with a non-zero status code). Otherwise, it exits with
the status code of 0, meaning that all the difftests found
were clean after analysis.

### `--action=rerun-dirty`

This action analyzes all the difftest directories, and then
checks the analysis results for the dirty tests. It then
proceeds to invoke an external command (`--runner` option)
for all the tests that have to be rerun, and then exits with
the status code of that external command. Projects with non-trivial
test pipelines can write special binaries for this purpose, but
the default of `cargo-difftests-default-runner` should be enough
for most projects. Writing a custom runner will be covered in a
later section.

## `cargo-difftests-default-runner`

As previously mentioned, this is the default runner that
`cargo-difftests` uses. It is a binary that is installed
alongside `cargo-difftests` by default.

It has a few requirements to be able to use:
- It has to be in the `PATH` environment variable.
- It has to be able to find the `cargo` binary in the `PATH` environment variable.
- There has to be a cargo profile setup specifically set-up for `cargo-difftests`
  (refer to [manual setup](Manual-setup.md#creating-the-difftests-profile) if you
  think you might not have that, the `cargo-generate` template contains one).
- The `extra` field passed to the `init` function of the `testclient` has to have
  the following fields:
  - `pkg_name` (a `String`): The name of the package that the test is in.
  - `test_name` (a `String`): The name of the test.

### Extra configuration

It then proceeds to invoke the following for each test.

```Bash
cargo test --profile <cargo_difftests_profile> --package <pkg_name> <test_name> <extra_args> -- --exact
```

- `<cargo_difftests_profile>` is the profile that `cargo-difftests` uses to run tests.
  By default, it is `difftests`, but can be configured using the `CARGO_DIFFTESTS_PROFILE`
  environment variable.
- `<pkg_name>` is the name of the package that the test is in.
- `<test_name>` is the name of the test.
- `<extra_args>` is a list of extra args that can be passed to `cargo test`,
  specified in the `CARGO_DIFFTESTS_EXTRA_CARGO_ARGS` environment variable. They are separated
  by `,`s, and then are passed to `cargo test` as-is.

## Custom test runners

If the default runner is not enough for your project, you can write your own.

Take a look over [the source code of the default runner][default-runner-source] if
you would like some inspiration, but the gist of it is that you have to write a rust
binary, which roughly looks like this:

```Rust
#[derive(serde::Deserialize)]
// fields have to be deserializable from the value passed to
// the `extra` field to the testclient `init` function.
struct TestExtra {
    test_name: String,
}

fn rerunner(
    invocation: cargo_difftests::test_rerunner_core::TestRerunnerInvocation
) -> T { // T can be anything, but it has to implement std::process::Termination
    // rerun invocation tests:
    for test in invocation.tests() {
        // do something with the test
        
        // parse the extra field
        let TestExtra {test_name} = test.parse_extra::<TestExtra>();
        
        // rerun the test
        let status = std::process::Command::new("hyper-complex-test-runner")
            .arg(test_name)
            .status()
            .expect("failed to run hyper-complex-test-runner");

        if !status.success() {
            std::process::exit(1);
        }
    }

    // create T
    T::default()
}

cargo_difftests::cargo_difftests_test_rerunner!(rerunner); // will create main
// which takes care of parsing the invocation and calling rerunner
```

> Keep in mind that the tests have to be rerun with one of the `profile`s which
> use `cargo-difftests`. If you do not do that, then the tests will be rerun,
> but no new coverage data will be collected, and `cargo-difftests` will not know
> that the tests were rerun, so it will keep saying that those tests are dirty,
> even if they are not. So remember to use the right profile, or `cargo-difftests`
> will be very confused.
{style="warning"}

[AnalyzeAllSingleTestGroup]: https://docs.rs/cargo-difftests/latest/cargo_difftests/struct.AnalyzeAllSingleTestGroup.html
[default-runner-source]: https://github.com/dnbln/cargo-difftests/blob/trunk/cargo-difftests/src/bin/cargo-difftests-default-rerunner.rs
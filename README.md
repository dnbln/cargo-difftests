# cargo-difftests

> "Insanity is doing the same thing over and over again and expecting different results."
>
> &#8212; Unknown author.

`cargo-difftests` is a tool that uses LLVM coverage data + file system mtimes
/ git diff information to find which tests have to be rerun.

## Prerequisites

- [`cargo-binutils`](https://github.com/rust-embedded/cargo-binutils)

## Cargo config setup

First, before we are able to do anything, we need a special profile
to output the coverage information, as well as a flag to enable the
library, so it's not enabled by mistake. Go ahead and copy the
following to your `.cargo/config.toml`:

```toml
# .cargo/config.toml
[profile.difftests] # you can name the profile whatever you want
inherits = "dev"
rustflags = [
  "-C", "instrument-coverage", # flag required for instrumentation-based code coverage
  "--cfg", "cargo_difftests", # cfg required for cargo-difftests-testclient, 
                              # more on it in a second
]

[env]
RUST_TEST_THREADS = "1" # we need this for profiling to work properly
# see comments on multithreading below

[unstable]
profile-rustflags = true
```

Now, the project has 2 main components:
- `cargo-difftests-testclient`
- `cargo-difftests`

## `cargo-difftests-testclient`

The test client is a bit of code that has to be run before each test.
It is given a basic description of the test (test name, which test
binary it comes from, and a couple of other bits of information to
help identify the test). This project explicitly relinquishes the responsibility
of actually identifying and reruning the test, but the information passed
to the testclient is usually more than enough to identify and rerun it if needed.

Besides that, it also sets up the llvm instrumentation runtime so that it will
write the profiling data to the temporary directory passed to the `init`
method, as well as giving the corresponding environment that has to be used
on child processes to do the same.

An example usage of the testclient library could be as follows (taken from the sample directory, which has a full sample project set up for `cargo-difftests`):

```rust
#[cfg(cargo_difftests)]
type DifftestsEnv = cargo_difftests_testclient::DifftestsEnv;

#[cfg(not(cargo_difftests))]
type DifftestsEnv = ();

#[must_use]
fn setup_difftests(group: &str, test_name: &str) -> DifftestsEnv {
  #[cfg(cargo_difftests)] // the cargo_difftests_testclient crate is empty 
  // without this cfg
  {
    // the temporary directory where we will store everything we need.
    // this should be passed to various `cargo difftests` subcommands as the 
    // `--dir` option.
    let tmpdir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("cargo-difftests").join(group).join(test_name);
    let difftests_env = cargo_difftests_testclient::init(
      cargo_difftests_testclient::TestDesc {
        // a "description" of the test.
        // cargo-difftests doesn't care about what you put here 
        // (except for the bin_path field) but it is your job to use
        // the data in here to identify the test
        // and rerun it if needed.
        // those fields are here to guide you, but you can add any other
        // fields you might need (see the `other_fields` field below)
        pkg_name: env!("CARGO_PKG_NAME").to_string(),
        crate_name: env!("CARGO_CRATE_NAME").to_string(),
        bin_name: option_env!("CARGO_BIN_NAME").map(ToString::to_string),
        bin_path: std::env::current_exe().unwrap(),
        test_name: test_name.to_string(),
        other_fields: std::collections::HashMap::new(), // any other 
        // fields you might want to add, to identify the test.
        // (the map is of type HashMap<String, String>)
      },
      &tmpdir,
    ).unwrap();
    // the difftests_env is important, because its Drop impl has
    // some cleanup to do (including actually writing the profile).
    //
    // if spawning children, it is also needed to
    // pass some environment variables to them, like this:
    //
    // cmd.envs(difftests_env.env_for_children());
    difftests_env
  }
}

#[test]
fn test_fn() {
  let _env = setup_difftests("dummy_group", "test_fn");
  assert_eq!(1 + 1, 2);
}
```

Keep in mind that only the profiling data for the code between the call to
`cargo_difftests_testclient::init` and the `drop()` of the value it returns
is kept track of.

## `cargo-difftests`

After all the tests have been run, the profiling data has to be interpreted,
and we have to check that the files that contain the code that was ran have
not modified since we last ran the test. If the code that is used in the test
has been modified since the last run, then we would consider the test "dirty",
and we should run it again, to make sure that it still passes after our changes.

All of that analysis happens in this crate.

To put it simply, `cargo-difftests` looks at the files that were "touched"
by the test, as in, during the execution, at least one line of code in the
given file was executed. If any of those files was modified, then it flags
the test as dirty and outputs said verdict. If none of the files were
modified, then re-running the test will not change the results, so the
verdict will be that the test is clean.

So you now can run the tests, maybe change a few files, and run a command
like the following:

```sh
cargo difftests analyze-all --dir target/tmp/cargo-difftests # common ancestor of all the difftest directories passed to the testclient init function
```

This will throw out a JSON array, containing the information
about the tests, as well as if they have to be rerun
(`"verdict": "dirty"`).

## Features

### Algorithms (`--algo` flag)

There are multiple ways to tell whether a change to a file
will impact the result of a test.

#### `fs-mtime`

The most basic way is to check the `mtime` of the files
which included executed code, comparing it with the time
we ran the test. This works well in most cases, and is
the default.

#### `git-diff-files`

Basically the same thing, but we assume that the last
full test run was at the last commit (good for CI), and
we compare the trees between the last commit and the
current state of the tree to check which files have
changed.

Also supports comparing with a given commit instead
of the last, but it should be passed as the
`--commit` option.

**Caution:** Running tests with a dirty working tree may cause
problems. As such, it is recommended to only use this on CI to
tell developers quickly about the results of the most-likely-affected
tests, but while actually working it would be wise to just use `fs-mtime`.

#### `git-diff-hunks`

This one expands on the `git-diff-files` algorithm,
but here, instead of checking if the file was touched
since the last commit, we check only for the specific
parts of the file that were modified. This works because
git actually keeps track of individual lines for us.

Also similarly to `git-diff-files`, this algorithm also
accepts an optional `--commit`, with which to compare
instead of the last commit.

**Caution:** Running tests with a dirty working tree may cause
problems. As such, it is recommended to only use this on CI to
tell developers quickly about the results of the most-likely-affected
tests, but while actually working it would be wise to just use `fs-mtime`.

### Groups

There are some projects which have many many tests in their test suites, and
storing profiling data for all of them can take up quite a lot of disk space.
In such cases, the tests can be grouped together into smaller units, and when
the code used in any of them changes, all the tests in the group can be rerun.

#### How to achieve grouping

There are two main ways to go about grouping tests together to achieve the above
effect.

##### Running all the tests of a group in a single process

In such cases, the `cargo_difftests_testclient::init` has to be called only
once. This could be achieved by use of a static variable, or something else
to do that. If a process doesn't finish before another call to the 
`cargo_difftests_testclient::init` function, the old directory might not be
interpretable by `cargo-difftests` when it's time to analyze it.

As above, `cargo-difftests` only cares about the `bin_path` field passed to
the test client, and ignores everything else. One could give those other
fields other meanings than intended, but it is also their job to decode them
when its time to rerun the tests.

##### Grouping the profiling data in directories

`cargo-difftests analyze-group` will treat *all* the valid directories under
the root directory it is passed as part of a "group" and therefore to be
analyzed together. This approach still keeps around all the original
profiling data, but there won't be more intermediary by-products of
the analysis than one per group, as opposed to one per test.

## Multi-threading

The way LLVM instrumentation works doesn't really lend itself well to the
multi-threading of tests, so multiple tests cannot run at the same time.
But a single test can spawn multiple threads, and the runtime will keep
track of those threads as well.
However, those extra threads *must* finish running before the test itself 
finishes. If they do not, and they are still going while the next test is
executing, they *will* interfere with that test's coverage.

This is not a problem if all the test code is single-threaded; however,
special caution should be taken for multi-threaded tests.

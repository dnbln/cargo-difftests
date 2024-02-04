# Unreleased

## Create an index right after a test finishes

With the `compile-index-and-clean` feature of the testclient,
it can invoke `cargo-difftests` to create an index immediately
after writing the profile, and then cleaning the intermediate
profiling data. This only happens if the testclient is configured
to do so (see `DifftestsEnv::and_compile_index_and_clean_on_exit`).

This, together with the tiny indexes feature released in `0.5.0`,
have the effect of a huge reduction in the on-disk space required
to store profiling data, only a couple KBs/test (depending on project
of course; on the sample project, the average is somewhere around 700
bytes / test).

# 0.5.0

Released: 2023-02-03

## Major features

### Dirty test rerunners

Basically executables that get invoked to rerun the tests which
are considered `Dirty`, together with a default impl for really
basic `cargo test`.

### Tiny indexes by default

A feature that strips away all the region-related coverage data
from an index, significantly reducing its size. They only contain
information about the "touched" files, but not about the specific
regions, and as such, can only be used with the `fs-mtime` and
`git-diff-files` algorithms; attempting to use it with `git-diff-hunks`
will cause an error.

## Other changes

- Move `rustc-wrapper-difftests` binaries into `cargo-difftests` package.

# 0.4.0

Released: 2023-01-10

## Breaking changes

The `TestDesc` now accepts any serializable type for the `extra` field,
and the fields which weren't needed are now gone.

## Other improvements

Improved docs and fixed doctests.

### `rustc-wrapper-difftests`

`rustc-wrapper-difftests` changes rustc invocation flags when
using difftests, so that rustc only enables coverage
instrumentation for crates within the workspace.

See [the section of the README](README.md#rustc-wrapper-difftests) for more.

# 0.3.0

## Breaking changes
Now the saving of the profiling data occurs during the `Drop`
implementation of the value returned from
`cargo_difftests_testclient::init`.

This change allows the use of the standard testing framework,
without the constraint that tests have to be run one-per-process.

This means that `cargo t --profile difftests` will run all the tests
and accurately record all the profiling data associated with them.

There is not perfect yet, as the tests cannot be run in parallel now;
they would mess with each other's counters if they do, so the tests
have to be ran sequentially. Tests can however spawn other threads 
(see comments on multithreading in the [README](README.md)).

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

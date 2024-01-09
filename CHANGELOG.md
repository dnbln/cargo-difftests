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
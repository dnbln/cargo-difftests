# cargo-difftests

> "Insanity is doing the same thing over and over again and expecting different results."
>
> &#8212; Unknown author.

`cargo-difftests` is a selective re-testing framework for rust.
To put it simply, it is a tool that uses LLVM coverage data +
some information about what has changed since the last test-run
to find which tests are most likely to have been affected by those
changes, and therefore need to be rerun.

The underlying assumption is that if a test passed in the past,
and none of the code executed in the test has been changed since,
then the result of the test will not change. While there are some
edge cases to this, it is generally true for most crates out there.

## Prerequisites

- Nightly rust.
- [`cargo-binutils`](https://github.com/rust-embedded/cargo-binutils)

## Recommended setup (with `cargo-generate`)

```bash
cargo install cargo-generate # if you don't have it already
```

Then just apply the template:

```bash
cargo generate dnbln/cargo-difftests
```

## Usage

A simple use of the `cargo difftests` now is as follows (in the template repository):

```bash
% # collect profiling data
% cargo difftests collect-profiling-data
% touch src/advanced_arithmetic.rs # change mtime
% cargo difftests analyze --dir target/tmp/difftests/tests/test_add
clean
% cargo difftests analyze --dir target/tmp/difftests/tests/test_mul
dirty
% cargo difftests analyze --dir target/tmp/difftests/tests/test_div
dirty
% cargo difftests collect-profiling-data --filter test_mul --exact
% cargo difftests analyze --dir target/tmp/difftests/tests/test_mul
clean
% cargo difftests analyze --dir target/tmp/difftests/tests/test_div
dirty
% cargo difftests collect-profiling-data --filter test_div --exact
% cargo difftests analyze --dir target/tmp/difftests/tests/test_div
clean
```

As you can see, it is quite verbose, but there is a command to simplify
things: `rerun-dirty-from-index`. This command takes a directory where
some indexes were stored, analyzes each of them, and then reruns the
tests that would be considered dirty.

A possible workflow would be:

```bash
# initial profiling data collection
cargo difftests collect-profiling-data --compile-index --index-root=difftests-index-root --root=target/tmp/difftests
```

```bash
# modify some files
touch src/lib.rs
```

```bash
# analyze and rerun tests, then recompile indexes
CARGO_DIFFTESTS_EXTRA_ARGS='--compile-index,--index-root=difftests-index-root,--root=target/tmp/difftests' cargo difftests rerun-dirty-from-indexes --index-root=difftests-index-root
```

This will initiall collect the profiling data for all tests, then compile
some indexes into the `difftests-index-root` directory, and then after some
changes have occured, the `rerun-dirty-from-indexes` command will rerun the
dirty tests, and compile new indexes from the new data in the same directory.

This is the recommended workflow to work with `cargo-difftests`. You might
want to create some aliases for those commands and/or put them in shell files
to make them simpler to work with.

### `cargo-difftests`

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
modified, then re-running the test will likely not change the results, so the
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

# Setup with cargo-generate (Recommended)

This tutorial goes over the recommended way to set up a new project to use with `cargo-difftests`.

If you would like to set it up manually on a bigger project, follow the tutorial on how to do
that [here](Manual-setup.md).

In this tutorial, you will learn how to:

* Set up a basic project using the `cargo-generate` template.
* Learn how to use the basic `cargo-difftests analyze` command in this template project to find dirty tests.

## Before you start

Make sure that:

- You understand what you're getting yourself into. Have you read the [overview](Overview.md)?
- You have [nightly rust](https://rust-lang.github.io/rustup/concepts/channels.html) installed.
- You have [`cargo-binutils`](https://github.com/rust-embedded/cargo-binutils)
  and [`cargo-generate`](https://github.com/cargo-generate/cargo-generate) installed.
- You also have `cargo-difftests` installed:

```Bash
cargo install cargo-difftests
```

- Optionally, if you choose the `workspace-only` option for coverage, you will additionally
  need `rustc-wrapper-difftests`:

```Bash
cargo install rustc-wrapper-difftests
```

## Applying the template

```Bash
cargo generate dnbln/cargo-difftests cargo-generate-templates/cargo-difftests-sample-project -d internal=false
```

Then follow `cargo-generate`'s prompts, and you will have yourself a brand-new starter project set up to work
with `cargo-difftests`.

## How to use `cargo-difftests`

The template project is set up with a new [cargo profile](https://doc.rust-lang.org/cargo/reference/profiles.html)
called `difftests`. When tests are ran with this profile, we pass a few special flags to `rustc` to have it emit
coverage instrumentation. In practice, all you have to do is write the following command to run your tests, just with
a `--profile difftests` flag to `cargo` so it knows to use the correct profile:

```Bash
cargo test --profile difftests
```

Now you should see the tests run as normal, just maybe a slight bit slower.

The tests in the template are nothing extraordinary: they just test a few functions that do basic arithmetic. What we
care about right now is that those functions are split into 2 files:

- <path>src/lib.rs</path>: contains `add` and `sub`.

- <path>src/advanced_arithmetic.rs</path>: contains `mul`, `div_unchecked` and `div`.

And the tests are written in <path>tests/tests.rs</path>.

Now we can finally invoke `cargo-difftests` for the first time.

```Bash
cargo difftests analyze-all
```

This instructs `cargo-difftests` to analyze all the profiling data
it was able to find, relating to our tests.

It should take a couple of seconds, but if you selected `all` for the coverage prompt while creating the template, it
might take a bit longer.

After all of that, it should output a big JSON with all the information it found. We can ask it about information for a
specific test like so:

```Bash
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_add
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_sub
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_mul
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div_2
clean
```

Now let us touch a file and see what happens:

```Bash
> touch src/lib.rs
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_add
dirty
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_sub
dirty
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_mul
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div_2
clean
```

The output for `test_add` and `test_sub` has changed from `clean` to `dirty`. This means that `cargo-difftests`
understood that the file might have changed (after all, we changed its mtime, and that is what it uses to track
by default). Let us rerun those tests now:

```Bash
> cargo test --profile difftests test_add
> cargo test --profile difftests test_sub
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_add
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_sub
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_mul
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div_2
clean
```

And now if we were to modify <path>src/advanced_arithmetic.rs</path>.

```Bash
> touch src/advanced_arithmetic.rs
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_add
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_sub
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_mul
dirty
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div
dirty
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div_2
dirty
> # rerun tests
> cargo test --profile difftests test_mul
> cargo test --profile difftests test_div -- --exact
> cargo test --profile difftests test_div_2
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_add
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/simple/test_sub
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_mul
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div
clean
> cargo difftests analyze --dir target/tmp/cargo-difftests/advanced/test_div_2
clean
```

{collapsible="true" collapsed-title="With src/advanced_arithmetic.rs"}

## What you've learned {id="what-learned"}

Congratulations! You now understand the basics of `cargo-difftests`, as well as how to set up a new project to use it
from the template.

### Where to go from here

[//]: # (<seealso>)

[//]: # (<category ref="settings">)

- [Go over the how to write tests section here.](Manual-setup.md#how-to-write-tests-for-cargo-difftests)
- [How to use the `analyze-all` subcommand to automatically find all tests and analyze them.](Use-with-analyze-all.md)

[//]: # (</category>)

[//]: # (</seealso>)

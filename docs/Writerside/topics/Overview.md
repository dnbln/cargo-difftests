# Overview

> "Insanity is doing the same thing over and over again and expecting different results."
>
> &#8212; Unknown author.

> Documentation currently under construction.
{style="warning"}

`cargo-difftests` is a [selective re-testing framework][selective-retesting-wikipedia] for rust.
To put it simply, it is a tool that uses LLVM coverage data +
some information about what has changed since the last test-run
to find which tests are most likely to have been affected by those
changes, and therefore need to be rerun.

The underlying assumption is that if a test passed in the past,
and none of the code executed in the test has been changed since,
then the result of the test will not change. While there are some
edge cases to this, it is generally true for most crates out there.

## What is `cargo-difftests`?

Until now, changes to source code pretty much always involved
rerunning the entire test suite. This can be quite time-consuming
for big projects with lots of tests, and beginner contributors
are often unaware of which tests could be influenced by their
newly-made changes.

This project attempts to do just that: point out the tests
whose results would most likely change, given some modifications
to the source code of the project.

## Glossary

First Term
: This is the definition of the first term.

Second Term
: This is the definition of the second term.


[selective-retesting-wikipedia]: https://en.wikipedia.org/wiki/Regression_testing#Regression_test_selection
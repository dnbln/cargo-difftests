# Overview

> "Insanity is doing the same thing over and over again and expecting different results."
>
> &#8212; Unknown author.

> Documentation currently under construction.
{style="warning"}

`cargo-difftests` is a tool that uses LLVM coverage data +
file system mtimes / git diff information to find which tests
have to be rerun. We assume tests where none of the *executed*
files changed, would not change their results, and as such, they
don't have to be rerun.

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

# On multithreading

The way LLVM instrumentation works doesn't really lend itself well to the
multi-threading of tests, so multiple tests cannot run at the same time.
But a single test can spawn multiple threads, and the runtime will keep
track of those threads as well.

However, those extra threads *must* finish running before the test itself
finishes. If they do not, and they are still going while the next test is
executing, they *will* interfere with that test's coverage.

This is not a problem if all the test code is single-threaded; however,
special caution should be taken for multi-threaded tests.

This however is different from spawning different processes, which is
supported; see the article on spawning subprocesses that should be tracked
[here](spawning-subprocesses.md).

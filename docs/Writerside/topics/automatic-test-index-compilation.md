# Automatic test index compilation

The `testclient` is able to immediately write out the profiling data for
the test it is running, and then invoke `cargo difftests` to compile the
index, and immediately delete the profiling data. This is useful for
projects that have a lot of tests, and don't want all the profiling data,
as it can take up a lot of space.

The `testclient` can be configured to do this by calling the
"compile-index-and-clean" feature, and then in the `setup_difftests`
function:

```Rust
let difftests_env = cargo_difftests_testclient::init(
        ...
).unwrap();

let difftests_env = difftests_env.and_compile_index_and_clean_on_exit(|b| {
            // calling either `.roots()` or `.index_path()` is required,
            // otherwise it won't know where to put the index.
            
            // `.roots(a, b)` corresponds to the `--index-root a --root b` options for index
            // compilation in `cargo-difftests` 
            b.roots(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("index_root"), root_dir)
                .flatten_files_to(
                    cargo_difftests_testclient::compile_index_and_clean_config::FlattenFilesToTarget::RepoRoot,
                )
                // other options for index compilation go here
        });
difftests_env
```
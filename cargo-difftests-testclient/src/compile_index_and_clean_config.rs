/*
 *        Copyright (c) 2023-2024 Dinu Blanovschi
 *
 *    Licensed under the Apache License, Version 2.0 (the "License");
 *    you may not use this file except in compliance with the License.
 *    You may obtain a copy of the License at
 *
 *        https://www.apache.org/licenses/LICENSE-2.0
 *
 *    Unless required by applicable law or agreed to in writing, software
 *    distributed under the License is distributed on an "AS IS" BASIS,
 *    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *    See the License for the specific language governing permissions and
 *    limitations under the License.
 */

use std::path::PathBuf;

pub struct CompileIndexAndCleanConfig {
    difftests_dir: PathBuf,
    index_resolver: IndexResolver,
    other_bins: Vec<PathBuf>,
    flatten_files_to: Option<FlattenFilesToTarget>,
    remove_bin_path: bool,
    full_index: bool,
    #[cfg(windows)]
    path_slash_replace: bool,
}

pub enum FlattenFilesToTarget {
    RepoRoot,
}

enum IndexResolver {
    FromRoots {
        index_root: PathBuf,
        difftests_root: PathBuf,
    },
    Path {
        index_path: PathBuf,
    },
}

pub struct CompileIndexAndCleanConfigBuilder {
    difftests_dir: PathBuf,
    index_resolver: Option<IndexResolver>,
    other_bins: Vec<PathBuf>,
    flatten_files_to: Option<FlattenFilesToTarget>,
    remove_bin_path: bool,
    full_index: bool,
    #[cfg(windows)]
    path_slash_replace: bool,
}

impl CompileIndexAndCleanConfigBuilder {
    pub fn new(difftests_dir: PathBuf) -> Self {
        Self {
            difftests_dir,
            index_resolver: None,
            other_bins: Vec::new(),
            flatten_files_to: None,
            remove_bin_path: true,
            full_index: false,
            #[cfg(windows)]
            path_slash_replace: true,
        }
    }

    pub fn index_path(mut self, index_path: impl Into<PathBuf>) -> Self {
        match &mut self.index_resolver {
            Some(_) => panic!("index_path or roots already set"),
            None => {
                self.index_resolver = Some(IndexResolver::Path {
                    index_path: index_path.into(),
                });
            }
        }
        self
    }

    pub fn roots(mut self, index_root: impl Into<PathBuf>, difftests_root: impl Into<PathBuf>) -> Self {
        match &mut self.index_resolver {
            Some(_) => panic!("index_path or roots already set"),
            None => {
                self.index_resolver = Some(IndexResolver::FromRoots {
                    index_root: index_root.into(),
                    difftests_root: difftests_root.into(),
                });
            }
        }
        self
    }

    pub fn other_bins(mut self, other_bins: impl IntoIterator<Item = PathBuf>) -> Self {
        self.other_bins.extend(other_bins);
        self
    }

    pub fn flatten_files_to(
        mut self,
        flatten_files_to: impl Into<Option<FlattenFilesToTarget>>,
    ) -> Self {
        self.flatten_files_to = flatten_files_to.into();
        self
    }

    pub fn remove_bin_path(mut self, remove_bin_path: bool) -> Self {
        self.remove_bin_path = remove_bin_path;
        self
    }

    pub fn full_index(mut self, full_index: bool) -> Self {
        self.full_index = full_index;
        self
    }

    #[cfg(windows)]
    pub fn path_slash_replace(mut self, path_slash_replace: bool) -> Self {
        self.path_slash_replace = path_slash_replace;
        self
    }

    #[track_caller]
    pub fn build(self) -> CompileIndexAndCleanConfig {
        CompileIndexAndCleanConfig {
            difftests_dir: self.difftests_dir,
            index_resolver: self.index_resolver.unwrap(),
            other_bins: self.other_bins,
            flatten_files_to: self.flatten_files_to,
            remove_bin_path: self.remove_bin_path,
            full_index: self.full_index,
            #[cfg(windows)]
            path_slash_replace: self.path_slash_replace,
        }
    }
}

pub fn do_build_index_and_clean(config: &CompileIndexAndCleanConfig) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(env!("CARGO"));

    cmd.args(&[
        "difftests",
        "low-level",
        "test-client-compile-test-index-and-clean",
    ]);

    cmd.arg("--dir").arg(&config.difftests_dir);

    match &config.index_resolver {
        IndexResolver::FromRoots {
            index_root,
            difftests_root,
        } => {
            cmd.arg("--index-root").arg(index_root);
            cmd.arg("--root").arg(difftests_root);
            cmd.arg("--output").arg("resolve");
        }
        IndexResolver::Path { index_path } => {
            cmd.arg("--output").arg(index_path);
        }
    }

    if let Some(flatten_files_to) = &config.flatten_files_to {
        match flatten_files_to {
            FlattenFilesToTarget::RepoRoot => {
                cmd.arg("--flatten-files-to=repo-root");
            }
        }
    }

    if !config.remove_bin_path {
        cmd.arg("--no-remove-bin-path");
    }

    if config.full_index {
        cmd.arg("--full-index");
    }

    #[cfg(windows)]
    {
        if !config.path_slash_replace {
            cmd.arg("--no-path-slash-replace");
        }
    }

    for bin in &config.other_bins {
        cmd.arg("--bin").arg(bin);
    }

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = cmd.output()?;

    if !output.status.success() {
        use std::io::Write;

        println!("cargo-difftests test-client-compile-test-index-and-clean stdout:");
        std::io::stdout().write_all(&output.stdout)?;
        eprintln!("cargo-difftests test-client-compile-test-index-and-clean stderr:");
        std::io::stderr().write_all(&output.stderr)?;

        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "cargo difftests low-level compile-test-index failed",
        ));
    }

    Ok(())
}

pub struct RunOnDrop(CompileIndexAndCleanConfig);

impl RunOnDrop {
    pub(crate) fn new(config: CompileIndexAndCleanConfig) -> Self {
        Self(config)
    }
}

impl Drop for RunOnDrop {
    fn drop(&mut self) {
        do_build_index_and_clean(&self.0).unwrap();
    }
}

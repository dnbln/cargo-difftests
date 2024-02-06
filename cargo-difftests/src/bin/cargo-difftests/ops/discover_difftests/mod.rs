use std::path::PathBuf;

use cargo_difftests::bin_context::CargoDifftestsContext;
use clap::Parser;

use crate::{ops::core::discover_difftests, CargoDifftestsResult};

#[derive(Parser, Debug)]
pub struct DiscoverDifftestsCommand {
    /// The root directory where all the difftests were stored.
    ///
    /// This should be some common ancestor directory of all
    /// the paths passed to `cargo_difftests_testclient::init`.
    #[clap(long, default_value = "target/tmp/cargo-difftests")]
    dir: PathBuf,
    /// The directory where the index files were stored, if any.
    #[clap(long)]
    index_root: Option<PathBuf>,
    /// With this flag, `cargo-difftests` will ignore any incompatible difftest and continue.
    ///
    /// Without this flag, when `cargo-difftests` finds an
    /// incompatible difftest on-disk, it will fail.
    #[clap(long)]
    ignore_incompatible: bool,
}
impl DiscoverDifftestsCommand {
    pub fn run(
        self,
        ctxt: &cargo_difftests::bin_context::CargoDifftestsContext,
    ) -> CargoDifftestsResult {
        run_discover_difftests(ctxt, self.dir, self.index_root, self.ignore_incompatible)
    }
}

fn run_discover_difftests(
    ctxt: &CargoDifftestsContext,
    dir: PathBuf,
    index_root: Option<PathBuf>,
    ignore_incompatible: bool,
) -> CargoDifftestsResult {
    let discovered = discover_difftests(dir, index_root, ignore_incompatible)?;
    let s = serde_json::to_string(&discovered)?;
    println!("{s}");

    Ok(())
}

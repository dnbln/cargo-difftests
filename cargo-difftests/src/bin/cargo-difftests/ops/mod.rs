use clap::Parser;

use crate::{CargoDifftestsContext, CargoDifftestsResult};

mod core;

mod analyze;
mod analyze_all;
mod analyze_all_from_index;
mod analyze_group;
mod discover_difftests;
mod low_level;

#[derive(Parser, Debug)]
pub enum App {
    /// Discover the difftests from a given directory.
    DiscoverDifftests {
        #[clap(flatten)]
        cmd: discover_difftests::DiscoverDifftestsCommand,
    },
    /// Analyze a single difftest.
    Analyze {
        #[clap(flatten)]
        cmd: analyze::AnalyzeCommand,
    },
    /// Treats all the difftests found in the given directory as a single
    /// group, and analyzes them together.
    AnalyzeGroup {
        #[clap(flatten)]
        cmd: analyze_group::AnalyzeGroupCommand,
    },
    /// Analyze all the difftests in a given directory.
    ///
    /// This is somewhat equivalent to running `cargo difftests discover-difftests`,
    /// and then `cargo difftests analyze` on each of the discovered difftests.
    AnalyzeAll {
        #[clap(flatten)]
        cmd: analyze_all::AnalyzeAllCommand,
    },
    /// Analyze all the difftests in a given directory, using their index files.
    ///
    /// Note that this does not require the outputs of the difftests to be
    /// present on-disk, and can be used to analyze difftests that were
    /// run on a different machine (given correct flags when
    /// compiling the index).
    AnalyzeAllFromIndex {
        #[clap(flatten)]
        cmd: analyze_all_from_index::AnalyzeAllFromIndexCommand,
    },
    LowLevel {
        #[clap(subcommand)]
        cmd: low_level::LowLevelCommand,
    },
}
impl App {
    pub(crate) fn run(self, ctxt: &CargoDifftestsContext) -> CargoDifftestsResult {
        match self {
            App::DiscoverDifftests { cmd } => {
                cmd.run(ctxt)?;
            }
            App::Analyze { cmd } => {
                cmd.run(ctxt)?;
            }
            App::AnalyzeGroup { cmd } => {
                cmd.run(ctxt)?;
            }
            App::AnalyzeAll { cmd } => {
                cmd.run(ctxt)?;
            }
            App::AnalyzeAllFromIndex { cmd } => {
                cmd.run(ctxt)?;
            }
            App::LowLevel { cmd } => {
                cmd.run(ctxt)?;
            }
        }

        Ok(())
    }
}

#[derive(Parser, Debug)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
pub enum CargoApp {
    Difftests {
        #[clap(subcommand)]
        app: App,
    },
}

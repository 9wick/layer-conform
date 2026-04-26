//! CLI argument parsing.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "layer-conform", version, about = "Detect layer style deviations")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Compare a single function against a single golden (MVP).
    Check {
        /// Source file containing the function to check.
        #[arg(long)]
        file: PathBuf,
        /// Symbol name within `--file` to check.
        #[arg(long)]
        symbol: String,
        /// Golden in the form "<path>:<symbol>".
        #[arg(long)]
        golden: String,
    },
}

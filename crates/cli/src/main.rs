//! layer-conform CLI entrypoint.

mod args;
mod reporter;
mod runner;

use clap::Parser;

use crate::args::{Cli, Cmd};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Check { file, symbol, golden } => runner::run_check(file, &symbol, &golden),
    }
}

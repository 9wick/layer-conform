//! CLI argument parsing.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "layer-conform", version, about = "Detect layer style deviations")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Cmd>,

    /// Override every rule's threshold (0.0–1.0).
    #[arg(long, value_name = "N", global = true)]
    pub threshold: Option<f64>,

    /// Disable ANSI colors.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Check files for layer conformance (default when no subcommand given).
    Check {
        /// Optional explicit paths; defaults to all files matching applyTo globs.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,

        /// Show only this file's detail.
        #[arg(long, value_name = "FILE")]
        explain: Option<PathBuf>,
    },

    /// Generate a starter `.layer-conform.json`.
    Init {
        /// Overwrite an existing config file.
        #[arg(long)]
        force: bool,
    },

    /// Explain why a file is conformant or deviant against its rule(s).
    Why {
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

//! layer-conform CLI entrypoint.

mod args;
mod commands;
mod loader;
mod reporter;

use clap::Parser;

use crate::args::{Cli, Cmd};
use crate::commands::{check, check::CheckOpts, init, init::InitOpts, why, why::WhyOpts};

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Cmd::Init { force }) => init::run(InitOpts { force }),
        Some(Cmd::Why { file }) => why::run(WhyOpts {
            file,
            no_color: cli.no_color,
            json: cli.json,
        }),
        Some(Cmd::Check { paths, explain }) => check::run(CheckOpts {
            paths,
            explain,
            threshold: cli.threshold,
            no_color: cli.no_color,
            json: cli.json,
        }),
        None => check::run(CheckOpts {
            paths: vec![],
            explain: None,
            threshold: cli.threshold,
            no_color: cli.no_color,
            json: cli.json,
        }),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            std::process::exit(2);
        }
    }
}

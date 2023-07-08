use std::{path::PathBuf, process};

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::style::{Attribute, Color, SetForegroundColor};

mod commands;

#[derive(Debug, Parser)]
#[command(name = "ftl")]
#[command(about = "A FTL developer tools.", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Create a bootfs image.")]
    Mkbootfs {
        #[arg(help = "The input directory")]
        indir: PathBuf,
        #[arg(help = "The output file")]
        outfile: PathBuf,
    },
}

fn parse_and_run_command() -> Result<()> {
    better_panic::install();
    let args = Cli::parse();

    match args.command {
        Commands::Mkbootfs { indir, outfile } => {
            commands::mkbootfs::main(&indir, &outfile)?;
        }
    }

    Ok(())
}

pub fn main() {
    better_panic::install();
    if let Err(err) = parse_and_run_command() {
        eprintln!(
            "devtools: {}{}error:{} {}{}",
            Attribute::Bold,
            SetForegroundColor(Color::Red),
            SetForegroundColor(Color::Reset),
            err,
            Attribute::Reset,
        );
        process::exit(1);
    }
}

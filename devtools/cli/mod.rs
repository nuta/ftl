use clap::{Parser, Subcommand};

mod color;
mod commands;

pub use color::Color;

#[derive(Subcommand)]
pub enum Command {
    Autogen(commands::autogen::Args),
}

#[derive(Parser)]
#[command(author, version, about)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

pub fn parse() -> Args {
    Args::parse()
}

pub fn run_command(args: Args) -> anyhow::Result<()> {
    match args.command {
        Command::Autogen(args) => commands::autogen::main(args)?,
    }

    Ok(())
}

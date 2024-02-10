use clap::Parser;

mod commands;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    command: commands::Command,
}

fn main() {
    let args = Args::parse();
    commands::run(&args.command).expect("command failed");
}

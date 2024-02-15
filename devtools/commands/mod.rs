mod autogen;

#[derive(clap::Subcommand)]
pub enum Command {
    Autogen(autogen::Args),
}

pub fn run(command: &Command) -> anyhow::Result<()> {
    match command {
        Command::Autogen(args) => autogen::main(args)?,
    }

    Ok(())
}

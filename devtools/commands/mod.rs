mod generate_loader_crate;

#[derive(clap::Subcommand)]
pub enum Command {
    GenerateLoaderCrate(generate_loader_crate::Args),
}

pub fn run(command: &Command) -> anyhow::Result<()> {
    match command {
        Command::GenerateLoaderCrate(args) => generate_loader_crate::main(&args)?,
    }

    Ok(())
}

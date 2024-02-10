use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    #[arg(help = "The output directory")]
    outdir: PathBuf,
    #[arg(help = "The list of fiber names to include")]
    fibers: Vec<String>,
}

pub fn main(args: &Args) -> anyhow::Result<()> {
    Ok(())
}

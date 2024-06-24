use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use ftl_types::idl::IdlFile;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(index = 1)]
    idl_file: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let idl_file = File::open(args.idl_file)?;
    let protocol: IdlFile = serde_json::from_reader(&idl_file)?;
    println!("{:#?}", protocol);
    Ok(())
}

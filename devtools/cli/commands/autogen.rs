use anyhow::Result;
use clap::Parser;

use crate::spec::SpecDatabase;

#[derive(Parser)]
pub struct Args {}

pub fn main(_args: Args) -> Result<()> {
    let specs = SpecDatabase::load()?;
    Ok(())
}

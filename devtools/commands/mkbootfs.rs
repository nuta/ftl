use std::path::Path;

use anyhow::{bail, Result};

pub fn main(indir: &Path, outfile: &Path) -> Result<()> {
    if !indir.is_dir() {
        bail!("{} is not a directory", indir.display());
    }


    Ok(())
}

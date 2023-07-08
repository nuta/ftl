use std::{path::Path, fs::OpenOptions};

use anyhow::{bail, Result, Context};

pub fn main(indir: &Path, outfile: &Path) -> Result<()> {
    if !indir.is_dir() {
        bail!("{} is not a directory", indir.display());
    }

    let mut tmpfile = tempfile::NamedTempFile::new()
        .context("failed to create temporary file")?;

    // TODO:
    // write!(&mut tmpfile, BootfsHeader {
    //     magic: BOOTFS_MAGIC,
    //     num_entries: 0,
    // })?;

    tmpfile.persist(outfile).context("failed to persist file")?;
    Ok(())
}

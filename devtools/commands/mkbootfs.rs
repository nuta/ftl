use std::{fs::OpenOptions, io::Write, path::Path};

use anyhow::{bail, Context, Result};
use bootfs::{EntryType, BOOTFS_MAGIC, NAME_LEN_MAX};
use bytes::{BufMut, BytesMut};
use essentials::{
    alignment::{align_up, is_aligned},
};
use glob::glob;
use std::io::Read;

pub fn main(indir: &Path, outfile: &Path) -> Result<()> {
    if !indir.is_dir() {
        bail!("{} is not a directory", indir.display());
    }

    let mut entries = Vec::new();
    let pat = indir.join("*").to_str().unwrap().to_owned();
    for entry in glob(&pat).context("failed to list files")? {
        let path = entry?;
        if !path.is_file() {
            continue;
        }

        let mut filedata: Vec<u8> = Vec::new();
        OpenOptions::new()
            .read(true)
            .open(&path)
            .context("failed to open file")?
            .read_to_end(&mut filedata)
            .context("failed to read file")?;

        let padding = align_up(filedata.len(), 4096) - filedata.len();
        entries.push((path, filedata, padding));
    }

    let mut image = BytesMut::new();

    // BootFS header.
    image.put_u32_le(BOOTFS_MAGIC);
    image.put_u32_le(entries.len().try_into().unwrap());

    // Entries.
    let mut offset = 0;
    for (path, filedata, padding) in &entries {
        let name = path
            .file_name()
            .context("failed to strip prefix")?
            .to_str()
            .context("failed to convert to str")?
            .to_owned();

        // Subtract 1 for the null terminator.
        if name.as_bytes().len() > NAME_LEN_MAX - 1 {
            bail!("too long path: {}", path.display());
        }

        // size
        image.put_u32_le(filedata.len().try_into().unwrap());
        // offset
        image.put_u32_le(offset.try_into().unwrap());
        // entry_type
        image.put_u8(EntryType::File as u8);
        // name
        image.put_slice(name.as_bytes());
        image.put_bytes(0, NAME_LEN_MAX - name.as_bytes().len());

        offset += filedata.len() + padding;
        debug_assert!(is_aligned(offset as usize, 4096));
    }

    // File data.
    for (_, filedata, padding) in &entries {
        image.put_slice(&filedata);
        image.put_bytes(0, *padding);
    }

    let mut tmpfile = tempfile::NamedTempFile::new()
        .context("failed to create temporary file")?;
    tmpfile.write_all(&image)?;
    tmpfile.persist(outfile).context("failed to persist file")?;
    Ok(())
}

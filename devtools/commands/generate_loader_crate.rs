use std::{fs, path::PathBuf};

use anyhow::{bail, Context};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long, help = "The output directory")]
    outdir: PathBuf,
    #[arg(help = "The list of fiber names to include")]
    fibers: Vec<String>,
}

pub fn main(args: &Args) -> anyhow::Result<()> {
    if let Some(parent_dir) = args.outdir.parent() {
        if !parent_dir.exists() {
            bail!("parent directory {} does not exist", parent_dir.display());
        }
    }

    let dir = tempfile::TempDir::new()?;

    let mut lib_rs = String::new();
    lib_rs.push_str("#![no_std]\n");
    lib_rs.push_str("\n");
    lib_rs.push_str("pub const FIBER_INITS: &[fn()] = &[");
    lib_rs.push_str(
        args.fibers
            .iter()
            .map(|fiber| format!("{fiber}::main"))
            .collect::<Vec<String>>()
            .join(", ")
            .as_str(),
    );
    lib_rs.push_str("];\n");

    let mut cargo_toml = String::new();
    cargo_toml.push_str("[package]\n");
    cargo_toml.push_str("name = \"ftl_loader\"\n");
    cargo_toml.push_str("version = \"0.0.0\"\n");
    cargo_toml.push_str("edition = \"2021\"\n");
    cargo_toml.push_str("\n");
    cargo_toml.push_str("[lib]\n");
    cargo_toml.push_str("path = \"lib.rs\"\n");
    cargo_toml.push_str("\n");
    cargo_toml.push_str("[dependencies]\n");
    for fiber in &args.fibers {
        cargo_toml.push_str(&format!(
            "{fiber} = {{ path = \"../../fibers/{fiber}\" }}\n"
        ));
    }

    fs::write(dir.path().join("lib.rs"), lib_rs)?;
    fs::write(dir.path().join("Cargo.toml"), cargo_toml)?;

    if args.outdir.exists() {
        fs::remove_dir_all(&args.outdir).with_context(|| {
            format!(
                "failed to remove the existing crate at {}",
                args.outdir.display()
            )
        })?;
    }

    fs::rename(dir.path(), &args.outdir).with_context(|| {
        format!(
            "failed to move the created crate to {}",
            args.outdir.display()
        )
    })?;

    Ok(())
}

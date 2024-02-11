use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context};
use ftl_types::spec::FiberSpec;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long, help = "The output directory")]
    outdir: PathBuf,
    #[arg(help = "The list of fiber names to include")]
    fibers: Vec<String>,
}

struct Fiber<'a> {
    spec: FiberSpec<'a>,
}

struct Generator<'a> {
    crate_dir: &'a Path,
    fibers: &'a [Fiber<'a>],
}

impl Generator {
    pub fn new(crate_dir: &Path, fibers: &'a [Fiber<'a>]) -> Generator<'a> {
        Generator { crate_dir, fibers }
    }

    fn generate_fiber(&mut self, dir: &Path) -> anyhow::Result<()> {
        fs::create_dir(&dir)?;

        let mut mod_rs = String::new();
        mod_rs.push_str("struct Deps {\n");
        mod_rs.push_str("    // TODO: add dependencies\n");
        mod_rs.push_str("}\n");

        Ok(())
    }

    fn generate_fibers(&mut self) -> anyhow::Result<()> {
        let fibers_dir = self.crate_dir.join("fibers");
        fs::create_dir_all(fibers_dir)?;

        let mut mod_rs = String::new();
        for fiber in self.fibers {
            let fiber_dir = fibers_dir.join(fiber);
            mod_rs.push_str(&format!("pub mod {};\n", fiber));
            self.generate_fiber(&fiber_dir)?;
        }

        fs::write(fibers_dir.join("mod.rs"), mod_rs)?;
        Ok(())
    }

    pub fn generate(mut self) -> anyhow::Result<()> {
        let mut lib_rs = String::new();
        lib_rs.push_str("#![no_std]\n");
        lib_rs.push_str("\n");

        lib_rs.push_str("pub mod fibers;\n");
        self.generate_fibers()?;

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
        cargo_toml.push_str("name = \"ftl_autogen\"\n");
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

        fs::write(self.crate_dir.path().join("lib.rs"), lib_rs)?;
        fs::write(self.crate_dir.path().join("Cargo.toml"), cargo_toml)?;

        Ok(())
    }
}

pub fn main(args: &Args) -> anyhow::Result<()> {
    if let Some(parent_dir) = args.outdir.parent() {
        if !parent_dir.exists() {
            bail!("parent directory {} does not exist", parent_dir.display());
        }
    }

    let dir = tempfile::TempDir::new()?;

    let mut generator = Generator::new(&dir);
    generator.generate();

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

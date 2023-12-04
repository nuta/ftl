use std::{collections::HashMap, fs::File};

use anyhow::{Context, Result};
use serde::Deserialize;
use walkdir::WalkDir;

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    #[serde(flatten)]
    pub item: ExportedItem,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExportedItem {
    Channel,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct FiberSpec {
    pub name: String,
    pub deps: Vec<String>,
    pub exports: Vec<Export>,
}

pub struct SpecDatabase {
    fibers: HashMap<String, FiberSpec>,
}

impl SpecDatabase {
    pub fn load() -> Result<SpecDatabase> {
        let mut fibers = HashMap::new();
        for entry in WalkDir::new("fibers") {
            let entry = entry?;
            if entry.file_type().is_file() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "yaml" || ext == "yml" {
                        let file = File::open(path)
                            .with_context(|| format!("failed to open file: {}", path.display()))?;
                        let spec: FiberSpec = serde_yaml::from_reader(file)
                            .with_context(|| format!("failed to parse file: {}", path.display()))?;
                        log::info!("loaded fiber spec: {:#?}", spec);
                        fibers.insert(spec.name.clone(), spec);
                    }
                }
            }
        }

        Ok(SpecDatabase { fibers })
    }
}

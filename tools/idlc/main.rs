use std::fmt;
use std::fs::File;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use ftl_types::idl;
use ftl_types::idl::IdlFile;
use ftl_types::spec::Spec;
use ftl_types::spec::SpecFile;
use minijinja::context;
use minijinja::Environment;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Field {
    name: String,
    ty: String,
}

#[derive(Debug, Serialize)]
struct Message {
    name: String,
    msgid: isize,
    fields: Vec<Field>,
}

#[derive(Debug, Serialize)]
struct App {
    name: String,
    depends: Vec<String>,
}

fn resolve_type_name(ty: &idl::Ty) -> String {
    match ty {
        idl::Ty::Int32 => "i32".to_string(),
    }
}

struct CamelCase<'a>(&'a str);

impl fmt::Display for CamelCase<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut next_upper = true;
        for c in self.0.chars() {
            if c == '_' {
                next_upper = true;
            } else if next_upper {
                write!(f, "{}", c.to_ascii_uppercase())?;
                next_upper = false;
            } else {
                write!(f, "{}", c)?;
            }
        }
        Ok(())
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    idl_file: PathBuf,
    #[arg(long)]
    app_specs: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let idl_file = File::open(args.idl_file)?;
    let protocol: IdlFile = serde_json::from_reader(&idl_file)?;

    let mut messages = Vec::new();
    for (i, protocol) in protocol.protocols.iter().enumerate() {
        for rpc in &protocol.rpcs {
            let request_name = format!("{}Request", CamelCase(&rpc.name));
            let reply_name = format!("{}Reply", CamelCase(&rpc.name));

            messages.push(Message {
                name: request_name,
                msgid: i as isize, // TODO: derive a globally unique ID
                fields: rpc
                    .request
                    .fields
                    .iter()
                    .map(|f| {
                        Field {
                            name: f.name.clone(),
                            ty: resolve_type_name(&f.ty),
                        }
                    })
                    .collect(),
            });

            messages.push(Message {
                name: reply_name,
                msgid: i as isize, // TODO: derive a globally unique ID
                fields: rpc
                    .response
                    .fields
                    .iter()
                    .map(|f| {
                        Field {
                            name: f.name.clone(),
                            ty: resolve_type_name(&f.ty),
                        }
                    })
                    .collect(),
            });
        }
    }

    let mut apps = Vec::new();
    for spec_path in args.app_specs {
        let spec_file = File::open(&spec_path)
            .with_context(|| format!("failed to open {}", spec_path.display()))?;
        let spec: SpecFile = serde_json::from_reader(&spec_file)
            .with_context(|| format!("failed to parse {}", spec_path.display()))?;
        match spec.spec {
            Spec::App(app) => {
                apps.push(App {
                    name: spec.name,
                    depends: app.depends,
                });
            }
        }
    }

    let mut j2env = Environment::new();
    j2env
        .add_template("template", include_str!("templates/ftl_autogen.rs.j2"))
        .unwrap();

    let template = j2env.get_template("template")?;
    let lib_rs = template.render(context! {
        messages => messages,
        apps => apps,
    })?;

    println!("{}", lib_rs);

    Ok(())
}

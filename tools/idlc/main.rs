use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use ftl_types::idl::{self, IdlFile};
use minijinja::{context, Environment};
use serde::Serialize;


#[derive(Debug, Serialize)]
struct Field {
    name: String,
    ty: String,
}

#[derive(Debug, Serialize)]
struct Message {
    name: String,
    fields: Vec<Field>,
}

fn resolve_type_name(ty: &idl::Ty) -> String {
    match ty {
        idl::Ty::Int32 => "i32".to_string(),
        idl::Ty::String => "String".to_string(),
        _ => panic!("Unknown type: {:?}", ty),
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(index = 1)]
    out_dir: PathBuf,
    #[arg(index = 2)]
    idl_file: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let idl_file = File::open(args.idl_file)?;
    let protocol: IdlFile = serde_json::from_reader(&idl_file)?;

    let mut j2env = Environment::new();
    j2env
        .add_template("template", include_str!("templates/ftl_autogen.rs.j2"))
        .unwrap();

    let mut messages = Vec::new();
    for protocol in protocol.protocols {
        for rpc in protocol.rpcs {
            messages.push(Message {
                name: rpc.name.clone(),
                fields: rpc.request.fields.into_iter().map(|f| Field {
                    name: f.name,
                    ty: resolve_type_name(&f.ty),
                }).collect(),
            });
            messages.push(Message {
                name: rpc.name.clone(),
                fields: rpc.response.fields.into_iter().map(|f| Field {
                    name: f.name,
                    ty: resolve_type_name(&f.ty),
                }).collect(),
            });
        }
    }

    let template = j2env.get_template("template")?;
    let lib_rs = template.render(context! {
        messages => messages,
    })?;
    println!("{}", lib_rs);


    Ok(())
}

use std::fmt;
use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use ftl_types::idl::IdlFile;
use ftl_types::idl::{self};
use minijinja::context;
use minijinja::Environment;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct Field {
    name: String,
    ty: String,
}

#[derive(Debug, Serialize)]
struct VaField {
    name: String,
    ty: String,
}

#[derive(Debug, Serialize)]
struct Message {
    name: String,
    msgid: isize,
    fields: Vec<Field>,
    va_fields: Vec<VaField>,
}

fn visit_message(name: String, idl_message: &idl::Message, msgid: isize) -> Message {
    let mut fields = Vec::new();
    let mut va_fields = Vec::new();
    for f in &idl_message.fields {
        let type_name = match &f.ty {
            idl::Ty::Int32 => "i32",
            idl::Ty::Bytes { .. } => "::ftl_types::idl::BytesField",
            _ => panic!("Unknown type: {:?}", f.ty),
        };

        fields.push(Field {
            name: f.name.clone(),
            ty: type_name.to_string(),
        });

        if let idl::Ty::Bytes { capacity } = &f.ty {
            va_fields.push(VaField {
                name: format!("va_{}", f.name),
                ty: format!("[u8; {}]", capacity),
            });
        }
    }

    Message {
        name,
        msgid: 0, // TODO: derive a globally unique ID
        fields,
        va_fields,
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
    #[arg(index = 1)]
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
    for (i, protocol) in protocol.protocols.iter().enumerate() {
        for rpc in &protocol.rpcs {
            messages.push(visit_message(
                format!("{}Request", CamelCase(&rpc.name)),
                &rpc.request,
                i as isize,
            ));
            messages.push(visit_message(
                format!("{}Reply", CamelCase(&rpc.name)),
                &rpc.response,
                i as isize,
            ));
        }
    }

    let template = j2env.get_template("template")?;
    let lib_rs = template.render(context! {
        messages => messages,
    })?;

    println!("{}", lib_rs);

    Ok(())
}

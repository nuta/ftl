use std::env;
use std::fs;
use std::path::Path;

use minijinja::context;
use minijinja::Environment;

const AUTOGEN_TEMPLATE: &str = r"#



#";

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("autogen.rs");

    let mut j2env = Environment::new();
    j2env.add_template("autogen", AUTOGEN_TEMPLATE).unwrap();

    let autogen = j2env
        .get_template("autogen")
        .unwrap()
        .render(context! {
            // protocols => protocols,
        })
        .unwrap();

    fs::write(dest_path, &autogen).unwrap();
}

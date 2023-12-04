use crate::cli::Color;

mod cli;
mod spec;

fn main() {
    env_logger::init();
    let args = cli::parse();
    if let Err(err) = cli::run_command(args) {
        eprintln!(
            "{}ftl-devtools: {}error:{} {:?}{}",
            Color::bold(),
            Color::red(),
            Color::reset_fg(),
            err,
            Color::reset_all(),
        );
        std::process::exit(1);
    }
}

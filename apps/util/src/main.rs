use std::process::ExitCode;

mod commands;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: util <command> [args...]");
        return ExitCode::FAILURE;
    }

    let name = &args[1];
    match name.as_str() {
        "cat" => commands::cat::main(&args[2..]),
        "ls" => commands::ls::main(&args[2..]),
        "pwd" => commands::pwd::main(&args[2..]),
        "" => {
            eprintln!("usage: util <command> [args...]");
            ExitCode::FAILURE
        }
        _ => {
            eprintln!("util: unknown command: {name}");
            ExitCode::FAILURE
        }
    }
}

use std::process::ExitCode;

pub fn main(args: &[String]) -> ExitCode {
    if let Some(dir) = args.first() {
        if let Err(error) = std::env::set_current_dir(dir) {
            eprintln!("pwd: {dir}: {error}");
            return ExitCode::FAILURE;
        }
    }

    match std::env::current_dir() {
        Ok(path) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("pwd: {error}");
            ExitCode::FAILURE
        }
    }
}

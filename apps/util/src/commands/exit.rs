use std::process::ExitCode;

pub fn main(args: &[String]) -> ExitCode {
    let Some(status) = args.first() else {
        return ExitCode::SUCCESS;
    };

    match status.parse::<u8>() {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            eprintln!("exit: {status}: {error}");
            ExitCode::FAILURE
        }
    }
}

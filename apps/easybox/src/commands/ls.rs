use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::process::ExitCode;

pub fn main(args: &[String]) -> ExitCode {
    let mut stdout = std::io::stdout();
    let mut exit_code = ExitCode::SUCCESS;
    if args.is_empty() {
        if let Err(error) = ls(".", &mut stdout) {
            eprintln!("ls: {error}");
            exit_code = ExitCode::FAILURE;
        }
    } else {
        for dir in args {
            if let Err(error) = ls(dir, &mut stdout) {
                eprintln!("ls: {dir}: {error}");
                exit_code = ExitCode::FAILURE;
            }
        }
    }

    exit_code
}

fn ls(path: &str, stdout: &mut impl Write) -> std::io::Result<()> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotADirectory => {
            return writeln!(stdout, "{path}");
        }
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.as_encoded_bytes();
        if name.starts_with(b".") {
            continue;
        }

        stdout.write_all(name)?;
        stdout.write_all(b"\n")?;
    }

    Ok(())
}

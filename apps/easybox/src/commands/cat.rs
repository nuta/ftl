use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::process::ExitCode;

pub fn main(args: &[String]) -> ExitCode {
    let mut stdout = std::io::stdout();
    let mut exit_code = ExitCode::SUCCESS;
    for path in args {
        if let Err(error) = cat(path, &mut stdout) {
            eprintln!("cat: {path}: {error}");
            exit_code = ExitCode::FAILURE;
        }
    }

    exit_code
}

fn cat(path: &str, stdout: &mut impl Write) -> std::io::Result<()> {
    let mut file = File::open(path)?;
    let mut buf = [0; 4096];
    loop {
        let len = file.read(&mut buf)?;
        if len == 0 {
            return Ok(());
        }

        stdout.write_all(&buf[..len])?;
    }
}

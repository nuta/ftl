use std::process::ExitCode;

pub fn main(args: &[String]) -> ExitCode {
    println!("{}", args.join(" "));
    ExitCode::SUCCESS
}

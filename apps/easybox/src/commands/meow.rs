use std::process::ExitCode;

pub fn main(args: &[String]) -> ExitCode {
    let message = if args.is_empty() {
        "meow!".to_string()
    } else {
        args.join(" ")
    };

    let line = "-".repeat(message.chars().count() + 2);
    println!("     {line}");
    println!("    < {message} >");
    println!("     {line}");
    println!("         /");
    println!(" /\\_/\\  /");
    println!("( o.o )");
    println!("\\(___)");
    ExitCode::SUCCESS
}

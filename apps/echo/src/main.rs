fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(first) = args.next() {
        print!("{first}");
        for arg in args {
            print!(" {arg}");
        }
    }
    println!();
}

use core::fmt;

use crate::env::Env;

pub struct Printer<'a>(&'a dyn Env);

impl<'a> Printer<'a> {
    pub const fn new(io: &'a dyn Env) -> Self {
        Self(io)
    }
}

impl<'a> fmt::Write for Printer<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.print(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! warn {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!(
            $io,
            "[driver   ] \x1b[33mWARN\x1b[0m  {}",
            format_args!($($arg)+)
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!($io, "[driver   ] \x1b[31mERROR\x1b[0m  {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! trace {
    ($io:expr, $($arg:tt)+) => {{
        $crate::println!($io, "[driver   ] {}", format_args!($($arg)+));
    }};
}

#[macro_export]
macro_rules! println {
    ($io:expr) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        use $crate::print::Printer;
        writeln!(Printer::new($io)).ok();
    }};
    ($io:expr, $($arg:tt)*) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        use $crate::print::Printer;
        writeln!(Printer::new($io), $($arg)*).ok();
    }};
}

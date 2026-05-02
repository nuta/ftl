use core::fmt;

use crate::syscall::sys_console_write;

pub struct Printer;

struct Buffer<const N: usize> {
    buffer: [u8; N],
    tail: usize,
}

impl<const N: usize> Buffer<N> {
    pub const fn new() -> Self {
        Self {
            buffer: [0; N],
            tail: 0,
        }
    }

    pub fn push(&mut self, byte: u8) {
        if self.tail >= N {
            return;
        }

        self.buffer[self.tail] = byte;
        self.tail += 1;
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buffer[..self.tail]
    }

    pub fn clear(&mut self) {
        self.tail = 0;
    }
}

static BUFFER: spin::Mutex<Buffer<512>> = spin::Mutex::new(Buffer::new());

impl fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut buffer = BUFFER.lock();
        for byte in s.as_bytes() {
            buffer.push(*byte);
            if *byte == b'\n' {
                sys_console_write(buffer.as_slice());
                buffer.clear();
            }
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! println {
    () => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer).ok();
    }};
    ($fmt:expr) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, $fmt).ok();
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        #[allow(unused_imports)]
        use core::fmt::Write;
        writeln!($crate::print::Printer, $fmt, $($arg)*).ok();
    }};
}

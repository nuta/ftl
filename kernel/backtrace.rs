use core::fmt;

use arrayvec::ArrayVec;

use crate::arch;

/// Prints a backtrace.
pub fn backtrace() {
    println!("backtrace:\n{:?}", Backtrace::capture());
}

pub struct CapturedFrame {
    pub pc: usize,
}

pub struct Backtrace {
    pub trace: ArrayVec<CapturedFrame, 16>,
}

impl Backtrace {
    /// Returns a saved backtrace.
    #[track_caller]
    pub fn capture() -> Backtrace {
        let mut trace = ArrayVec::new();
        arch::backtrace(|pc| {
            trace.push(CapturedFrame { pc });
        });

        Backtrace { trace }
    }
}

impl fmt::Debug for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, frame) in self.trace.iter().enumerate() {
            let _ = writeln!(
                f,
                "  {start}bt:{index}:0x{vaddr:x}{end}",
                start = "{{{",
                end = "}}}",
                index = i,
                vaddr = frame.pc
            );
        }

        Ok(())
    }
}

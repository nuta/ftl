use alloc::sync::Arc;

use ftl::mutex::Mutex;

use crate::types::c_long;
use crate::types::c_short;
use crate::types::c_ulong;
use crate::types::errno::Errno;
use crate::types::sys::ioctl::ECHO;
use crate::types::sys::ioctl::ICRNL;
use crate::types::sys::ioctl::TCGETS;
use crate::types::sys::ioctl::TCSETS;
use crate::types::sys::ioctl::TCSETSF;
use crate::types::sys::ioctl::TCSETSW;
use crate::types::sys::ioctl::TIOCGWINSZ;
use crate::types::sys::ioctl::Termios;
use crate::types::sys::ioctl::WinSize;
use crate::types::sys::stat::S_IFCHR;
use crate::types::sys::stat::Stat;
use crate::vfs::FileLike;
use crate::vfs::addr_as_ino;
use crate::wait_queue::Sleep;
use crate::wait_queue::WaitQueue;

pub struct Tty {
    inner: Arc<dyn FileLike>,
    termios: Mutex<Termios>,
}

impl Tty {
    pub fn new(inner: Arc<dyn FileLike>) -> Self {
        Self {
            inner,
            termios: Mutex::new(Termios::new()),
        }
    }
}

impl FileLike for Tty {
    fn read(
        &self,
        buf: &mut [u8],
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        let n = self.inner.read(buf, offset, nonblocking, sleep)?;
        let termios = *self.termios.lock();
        for byte in &mut buf[..n] {
            if *byte == b'\r' && termios.c_iflag & ICRNL != 0 {
                *byte = b'\n';
            }

            // TODO: Implement cook mode.
            let printable = !byte.is_ascii_control() || *byte == b'\n' || *byte == b'\t';
            if termios.c_lflag & ECHO != 0 && printable {
                let _ = self.inner.write(&[*byte], 0, nonblocking, sleep);
            }
        }
        Ok(n)
    }

    fn write(
        &self,
        buf: &[u8],
        offset: usize,
        nonblocking: bool,
        sleep: Sleep<'_>,
    ) -> Result<usize, Errno> {
        self.inner.write(buf, offset, nonblocking, sleep)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        self.inner.poll()
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Ok(Stat::new(addr_as_ino(self), S_IFCHR | 0o620, 0))
    }

    fn ioctl(&self, cmd: c_ulong, arg: usize) -> Result<c_long, Errno> {
        if arg == 0 {
            return Err(Errno::EFAULT);
        }

        match cmd {
            TCGETS => {
                let termios = *self.termios.lock();
                unsafe { (arg as *mut Termios).write(termios) };
            }
            TCSETS | TCSETSW | TCSETSF => {
                *self.termios.lock() = unsafe { (arg as *const Termios).read() };
            }
            TIOCGWINSZ => {
                // A hard-coded value to make ioctl work.
                let winsize = WinSize {
                    ws_row: 24,
                    ws_col: 80,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };

                unsafe { (arg as *mut WinSize).write(winsize) };
            }
            _ => return Err(Errno::ENOTTY),
        }

        Ok(0)
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        self.inner.wait_queue()
    }
}

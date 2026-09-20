use alloc::sync::Arc;

use crate::types::c_short;
use crate::types::errno::Errno;
use crate::vfs::FileLike;
use crate::wait_queue::WaitQueue;

pub struct Tty {
    inner: Arc<dyn FileLike>,
}

impl Tty {
    pub fn new(inner: Arc<dyn FileLike>) -> Self {
        Self { inner }
    }
}

impl FileLike for Tty {
    fn read(&self, buf: &mut [u8], offset: usize, nonblocking: bool) -> Result<usize, Errno> {
        let n = self.inner.read(buf, offset, nonblocking)?;
        for byte in &mut buf[..n] {
            if *byte == b'\r' {
                *byte = b'\n';
            }

            // TODO: Implement cook mode.
            if !byte.is_ascii_control() || *byte == b'\n' || *byte == b'\t' {
                let _ = self.inner.write(&[*byte], 0, nonblocking);
            }
        }
        Ok(n)
    }

    fn write(&self, buf: &[u8], offset: usize, nonblocking: bool) -> Result<usize, Errno> {
        self.inner.write(buf, offset, nonblocking)
    }

    fn poll(&self) -> Result<c_short, Errno> {
        self.inner.poll()
    }

    fn wait_queue(&self) -> Option<&WaitQueue> {
        self.inner.wait_queue()
    }
}

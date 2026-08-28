use crate::dma::DmaBuf;

#[derive(Debug)]
pub struct OutOfMemoryError;

pub trait Env {
    fn alloc_dma(&self, size: usize) -> Result<DmaBuf, OutOfMemoryError>;
    fn free_dma(&self, buf: DmaBuf);
    fn print(&self, text: &str);

    #[cfg(target_arch = "x86_64")]
    unsafe fn out8(&self, port: u16, value: u8) {
        unsafe {
            core::arch::asm!("out dx, al", in("dx") port, in("al") value);
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn out16(&self, port: u16, value: u16) {
        unsafe {
            core::arch::asm!("out dx, ax", in("dx") port, in("ax") value);
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn out32(&self, port: u16, value: u32) {
        unsafe {
            core::arch::asm!("out dx, eax", in("dx") port, in("eax") value);
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn in8(&self, port: u16) -> u8 {
        let value: u8;
        unsafe {
            core::arch::asm!("in al, dx", in("dx") port, out("al") value);
        }
        value
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn in16(&self, port: u16) -> u16 {
        let value: u16;
        unsafe {
            core::arch::asm!("in ax, dx", in("dx") port, out("ax") value);
        }
        value
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn in32(&self, port: u16) -> u32 {
        let value: u32;
        unsafe {
            core::arch::asm!("in eax, dx", in("dx") port, out("eax") value);
        }
        value
    }
}

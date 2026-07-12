use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use crate::handle::Handle;
use crate::vmspace::PageAttrs;

static START_INFO: AtomicUsize = AtomicUsize::new(0);

pub struct StartInfo {
    pub malloc: fn(size: usize) -> crate::Result<*mut u8>,
    pub print: fn(bytes: &[u8]),
    pub panic: fn(),
    pub vmspace_create: fn() -> crate::Result<Handle>,
    pub vmarea_allocate: fn(len: usize) -> crate::Result<Handle>,
    pub vmarea_write: fn(vmarea: &Handle, offset: usize, data: &[u8]) -> crate::Result<()>,
    pub vmspace_map:
        fn(vmspace: &Handle, vmarea: &Handle, uaddr: usize, attrs: PageAttrs) -> crate::Result<()>,
}

pub fn start_info() -> &'static StartInfo {
    let ptr = START_INFO.load(Ordering::Relaxed);
    debug_assert!(ptr != 0);
    unsafe { &*(ptr as *const StartInfo) }
}

unsafe extern "Rust" {
    static SPEC: crate::Spec;
}

#[unsafe(no_mangle)]
pub fn server_start(start_info_ptr: *const StartInfo) {
    START_INFO.store(start_info_ptr as usize, Ordering::Relaxed);
    unsafe { (SPEC.start)() };
}

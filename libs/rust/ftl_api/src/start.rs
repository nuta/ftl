use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

static START_INFO: AtomicUsize = AtomicUsize::new(0);

pub struct StartInfo {
    pub print: fn(bytes: &[u8]),
    pub panic: fn(),
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

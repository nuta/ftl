use core::alloc::Layout;

use spin::Mutex;
use talc::Span;
use talc::Talc;
use talc::Talck;

struct OomHandler;

impl talc::OomHandler for OomHandler {
    fn handle_oom(_talc: &mut talc::Talc<Self>, layout: Layout) -> Result<(), ()> {
        panic!("out of memory: {:?}", layout);
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Talck<Mutex<()>, OomHandler> = Talc::new(OomHandler).lock::<Mutex<()>>();

#[repr(align(16))]
struct Aligned<T>(T);

const HEAP_SIZE: usize = 1024 * 1024; // 1 MB
static mut HEAP: Aligned<[u8; HEAP_SIZE]> = Aligned([0; HEAP_SIZE]);

pub(crate) fn init() {
    unsafe {
        let base = core::ptr::addr_of_mut!(HEAP) as *mut u8;
        let span = Span::new(base, base.add(HEAP_SIZE));
        GLOBAL_ALLOCATOR
            .lock()
            .claim(span)
            .expect("failed to initialize allocator");
    }
}

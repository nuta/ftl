use alloc::boxed::Box;

use ftl_types::error::FtlError;

use crate::println;

pub fn set_interrupt_handler<F>(f: F) -> Result<(), FtlError>
where
    F: FnOnce() + Send + Sync + 'static,
{
    extern "C" fn native_entry(arg: *mut Box<dyn FnOnce()>) {
        let closure = unsafe { Box::from_raw(arg) };
        closure();
    }

    let main = move || {
        f();
        panic!("interrupt handler has exited");
    };

    let pc = native_entry as usize;
    let closure = Box::into_raw(Box::new(main));
    let arg = closure as usize;
}

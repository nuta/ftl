use alloc::boxed::Box;

use ftl_types::error::FtlError;

use crate::syscall;

pub fn set_interrupt_handler<F>(f: F) -> Result<(), FtlError>
where
    F: FnOnce() + Send + Sync + 'static,
{
    let main = move || {
        f();
        panic!("interrupt handler has exited");
    };

    extern "C" fn entry(arg: *mut Box<dyn FnOnce()>) {
        let closure = unsafe { Box::from_raw(arg) };
        closure();
    }

    let pc = entry as usize;
    let closure = Box::into_raw(Box::new(main));
    let arg = closure as usize;

    syscall::interrupt_set_kernel_handler(pc, arg)
}

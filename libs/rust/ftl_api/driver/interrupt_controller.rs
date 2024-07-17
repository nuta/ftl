use alloc::boxed::Box;

use ftl_types::error::FtlError;

use crate::syscall;

pub fn set_interrupt_handler<F>(f: F) -> Result<(), FtlError>
where
    F: FnMut() + Send + Sync + 'static,
{
    extern "C" fn entry<F>(arg: usize) where F: FnMut() {
        let closure = unsafe {
            &mut *(arg as *mut F)
        };

        closure();
    }

    let pc = entry::<F> as usize;
    let closure = Box::into_raw(Box::new(f));
    let arg = closure as usize;

    syscall::interrupt_set_kernel_handler(pc, arg)
}

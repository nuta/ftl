use alloc::boxed::Box;
use core::marker::PhantomPinned;
use core::mem::MaybeUninit;

pub(crate) struct UserData<C: 'static, H: 'static> {
    pub ctx: C,
    pub handler: H,
    // Kernel holds a pointer to this, so it must be moved to somewhere else.
    _pin: PhantomPinned,
}

type Dispatcher<T> = extern "Rust" fn(user_data: usize, arg: T);

pub struct Upcall<T> {
    dispatch: Dispatcher<T>,
    user_data: usize,
}

unsafe impl<T> Send for Upcall<T> {}
unsafe impl<T> Sync for Upcall<T> {}

impl<T> Upcall<T> {
    pub(crate) fn new<F, H, C, E>(dispatch: Dispatcher<T>, handler: H, ctor: F) -> Result<C, E>
    where
        F: FnOnce(Upcall<T>) -> Result<C, E>,
        H: Send + Sync + 'static,
        C: Clone + Send + Sync + 'static,
    {
        // Allocate a memory space for the user data, but don't initialize it yet.
        let uninit = Box::new(MaybeUninit::<UserData<C, H>>::uninit());
        let ptr: *mut UserData<C, H> = Box::into_raw(uninit).cast();

        let upcall = Upcall {
            dispatch,
            user_data: ptr as usize,
        };

        // Call the constructor to create a kernel object with the upcall.
        let ctx = match ctor(upcall) {
            Ok(ctx) => ctx,
            Err(err) => {
                // Failed to create the kernel object. Free the memory space.
                drop(unsafe { Box::from_raw(ptr.cast::<MaybeUninit<UserData<C, H>>>()) });
                return Err(err);
            }
        };

        // Successfully created the kernel object. We can fill the context now.
        // FIXME: How can we guarantee that the kernel object won't access the handler?
        unsafe {
            ptr.write(UserData {
                ctx: ctx.clone(),
                handler,
                _pin: PhantomPinned,
            });
        }

        Ok(ctx)
    }
}

impl<T> Upcall<T> {
    pub fn invoke(&self, arg: T) {
        (self.dispatch)(self.user_data, arg)
    }
}

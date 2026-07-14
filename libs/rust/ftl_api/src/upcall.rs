use alloc::boxed::Box;
use core::marker::PhantomPinned;
use core::mem::MaybeUninit;

pub(crate) struct UserData<C: 'static, H: 'static> {
    pub object: C,
    pub handler: H,
    // Kernel holds a pointer to this, so it must be moved to somewhere else.
    _pin: PhantomPinned,
}

impl<C, H> UserData<C, H> {
    pub(crate) unsafe fn borrow<'a>(ctx: UpCallCtx) -> &'a Self {
        unsafe { &*(ctx.0 as *const Self) }
    }

    pub(crate) unsafe fn reclaim(ctx: UpCallCtx) -> Self {
        let user_data = unsafe { Box::from_raw(ctx.0 as *mut Self) };
        *user_data
    }
}

#[derive(Clone, Copy)]
pub struct UpCallCtx(usize);

type Dispatcher<T> = extern "Rust" fn(ctx: UpCallCtx, arg: T);

pub struct Upcall<T> {
    dispatch: Dispatcher<T>,
    ctx: UpCallCtx,
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
            ctx: UpCallCtx(ptr as usize),
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
                object: ctx.clone(),
                handler,
                _pin: PhantomPinned,
            });
        }

        Ok(ctx)
    }
}

impl<T> Upcall<T> {
    pub fn invoke(&self, arg: T) {
        (self.dispatch)(self.ctx, arg)
    }
}

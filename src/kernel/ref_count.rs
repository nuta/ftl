use core::{ptr::NonNull, ops::Deref};

pub unsafe trait RefCounted {
    fn inc_ref(&self);
    fn dec_ref(&self);
}

pub struct Ref<T: RefCounted + ?Sized> {
    ptr: NonNull<T>,
}

impl<T: RefCounted + ?Sized> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

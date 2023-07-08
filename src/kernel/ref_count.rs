use core::ptr::NonNull;

pub unsafe trait RefCounted: ?Sized {
    fn inc_ref(&self);
    fn dec_ref(&self);
}

pub struct Ref<T: RefCounted> {
    ptr: NonNull<T>,
}

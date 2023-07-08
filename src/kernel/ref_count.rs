use core::ptr::NonNull;

pub unsafe trait RefCounted: ?sized {
    fn inc_ref(&self);
    fn dec_ref(&self);
}

pub struct Ref<T: RefCounted> {
    ptr: NonNull<T>,
}

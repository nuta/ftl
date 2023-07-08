use core::ptr::NonNull;

pub unsafe trait RefCounted {
    fn inc_ref(&self);
    fn dec_ref(&self);
}

pub struct Ref<T: RefCounted + ?Sized> {
    ptr: NonNull<T>,
}

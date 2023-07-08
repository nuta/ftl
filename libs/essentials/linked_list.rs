use core::{
    marker::{PhantomData, PhantomPinned},
    pin::Pin,
    ptr::NonNull,
};

pub trait Adapter: Sized {
    type Container;
    fn container_to_link(container: &mut Self::Container) -> Pin<Link<Self>>;
    fn link_to_container(link: &mut Link<Self>) -> Self::Container;
}

pub struct Link<A: Adapter> {
    prev: Option<NonNull<Link<A>>>,
    next: Option<NonNull<Link<A>>>,
    _pd: PhantomData<A::Container>,
    _pin: PhantomPinned,
}

impl<A: Adapter> Link<A> {
    fn is_linked(&self) -> bool {
        self.prev.is_some() || self.next.as_ref().is_some()
    }
}

impl<A: Adapter> Drop for Link<A> {
    fn drop(&mut self) {
        assert!(!self.is_linked());
    }
}

pub struct LinkedList<A: Adapter> {
    head_and_tail: Link<A>,
}

impl<A: Adapter> LinkedList<A> {
    pub const fn new(self: Pin<&mut Self>) -> LinkedList<A> {
        let list_addr =
            unsafe { &self.get_unchecked_mut().head_and_tail as *const _ };
        LinkedList {
            head_and_tail: Link {
                prev: Some(unsafe {
                    NonNull::new_unchecked(list_addr as *mut _)
                }),
                next: Some(unsafe {
                    NonNull::new_unchecked(list_addr as *mut _)
                }),
                _pd: PhantomData,
                _pin: PhantomPinned,
            },
        }
    }

    fn head_and_tail(&mut self) -> (&Link<A>, &Link<A>) {
        // Safety: `head_and_tail` will never be None.
        unsafe {
            (
                self.head_and_tail.next.unwrap_unchecked().as_ref(),
                self.head_and_tail.prev.unwrap_unchecked().as_ref(),
            )
        }
    }

    fn head_and_tail_mut(&mut self) -> (&mut Link<A>, &mut Link<A>) {
        // Safety: `head_and_tail` will never be None.
        unsafe {
            (
                self.head_and_tail.next.unwrap_unchecked().as_mut(),
                self.head_and_tail.prev.unwrap_unchecked().as_mut(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MyStruct {
        link: Link<MyStructAdapter>,
        val: usize,
    }

    struct MyStructAdapter;
    impl Adapter for MyStructAdapter {
        type Container = MyStruct;
        fn container_to_link(
            container: &mut Self::Container,
        ) -> Pin<Link<Self>> {
            todo!()
        }

        fn link_to_container(link: &mut Link<Self>) -> Self::Container {
            // unsafe { &mut *link as *mut Link<Self> as *mut MyStruct }
            todo!()
        }
    }

    #[test]
    fn test_linked_list() {
        static mut list: LinkedList<MyStructAdapter> =
            LinkedList::new(Pin::static_mut(unsafe { &mut list }));
    }
}

use core::{marker::PhantomData, mem::size_of, ops::Deref, ptr};

use paste::paste;

use crate::{
    arch::{read_cpulocal_base, write_cpulocal_base},
    memory::PAGE_ALLOCATOR,
};

#[macro_export]
macro_rules! __cpu_local_inner {
    ($V:vis, $N:ident, $T:ty, $E:expr) => {
        paste! {
            #[used]
            #[link_section = ".cpu_local"]
            #[allow(non_upper_case_globals)]
            static [<$N _INIT>]: $T = $E;

            $V static $N: $crate::cpu_local::CpuLocal<$T> = CpuLocal::new(&[<$N _INIT>]);
        }
    };
}

/// Defines a CPU-local variable.
#[macro_export]
macro_rules! cpu_local {
    (static ref $N:ident : $T:ty = $E:expr ;) => {
        $crate::__cpu_local_inner!(, $N, $T, $E);
    };
    (pub static ref $N:ident : $T:ty = $E:expr ;) => {
        $crate::__cpu_local_inner!(pub, $N, $T, $E);
    };
}

extern "C" {
    static __cpu_local: u8;
    static __cpu_local_end: u8;
}

pub struct CpuLocal<T: 'static> {
    offset: Option<usize>,
    init: &'static T,
    _pd: PhantomData<T>,
}

impl<T> CpuLocal<T> {
    pub const fn new(init: &'static T) -> CpuLocal<T> {
        CpuLocal {
            offset: None,
            init,
            _pd: PhantomData,
        }
    }

    pub fn get(&self) -> &T {
        let offset = self.offset.unwrap_or_else(|| {
            let base;
            let end;
            unsafe {
                base = &__cpu_local as *const _ as usize;
                end = &__cpu_local_end as *const _ as usize;
            }
            let init_addr = self.init as *const _ as usize;

            debug_assert!(base <= init_addr && init_addr < end);
            init_addr - base
        });

        unsafe { &*((read_cpulocal_base() + offset) as *const T) }
    }
}

impl<T> Deref for CpuLocal<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.get()
    }
}

unsafe impl<T> Sync for CpuLocal<T> {}

cpu_local! {
    pub static ref bar: usize = 456;
}

#[inline(never)]
fn return_bar() -> usize {
    *bar
}

pub fn init_percpu() {
    let init_base = unsafe { &__cpu_local as *const _ as usize };
    let init_end = unsafe { &__cpu_local_end as *const _ as usize };
    let per_cpu_size = init_end - init_base;

    let percpu_base = PAGE_ALLOCATOR
        .get_mut()
        .allocate(per_cpu_size, size_of::<usize>())
        .unwrap();
    write_cpulocal_base(percpu_base.get());

    // copy initial values
    unsafe {
        ptr::copy_nonoverlapping(
            init_base as *const u8,
            percpu_base.get() as *mut u8,
            per_cpu_size,
        );
    }

    println!("deref = {}", return_bar());
}

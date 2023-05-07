use core::{
    marker::PhantomData,
    mem::size_of,
    ops::Deref,
    ptr::{self, addr_of},
};

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
///
/// # Examples
///
/// ```
/// cpu_local! {
///     pub static ref InterruptCounter: usize = 123;
/// }
/// ```
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
    init: &'static T,
    _pd: PhantomData<T>,
}

impl<T> CpuLocal<T> {
    pub const fn new(init: &'static T) -> CpuLocal<T> {
        CpuLocal {
            init,
            _pd: PhantomData,
        }
    }

    pub fn get(&self) -> &T {
        // TODO: Cache this offset value. Use Once<T>.
        let offset = {
            let init_base;
            let init_end;
            unsafe {
                init_base = addr_of!(__cpu_local) as usize;
                init_end = addr_of!(__cpu_local_end) as usize;
            }
            let init_addr = self.init as *const _ as usize;

            debug_assert!(init_base <= init_addr && init_addr < init_end);
            init_addr - init_base
        };

        unsafe { &*((read_cpulocal_base() + offset) as *const T) }
    }
}

impl<T> Deref for CpuLocal<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.get()
    }
}

/// Initializes the CPU-local variables. This function must be called
/// after the memory allocator is initialized and in each CPU initialization.
pub fn init_percpu() {
    let init_base = unsafe { addr_of!(__cpu_local) as usize };
    let init_end = unsafe { addr_of!(__cpu_local_end) as usize };
    let per_cpu_size = init_end - init_base;

    let percpu_base = PAGE_ALLOCATOR
        .get_mut()
        .allocate(per_cpu_size, size_of::<usize>())
        .unwrap();
    write_cpulocal_base(percpu_base.get());

    // Fill the percpu area with the initial values.
    unsafe {
        ptr::copy_nonoverlapping(
            init_base as *const u8,
            percpu_base.get() as *mut u8,
            per_cpu_size,
        );
    }
}

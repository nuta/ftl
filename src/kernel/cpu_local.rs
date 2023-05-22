use core::{
    cell::RefCell,
    mem::size_of,
    ops::Deref,
    ptr::{self, addr_of},
};

use crate::{
    arch::{read_cpulocal_base, write_cpulocal_base},
    memory::PAGE_ALLOCATOR,
};

#[macro_export]
macro_rules! __cpu_local_inner {
    ($V:vis, $N:ident, $T:ty, $E:expr) => {
        ::paste::paste! {
            #[used]
            #[link_section = ".cpu_local"]
            #[allow(non_upper_case_globals)]
            static [<$N _INIT>]: $crate::cpu_local::InitialValue<$T> = $crate::cpu_local::InitialValue::new_internal($E);

            $V static $N: $crate::cpu_local::CpuLocal<$T> = $crate::cpu_local::CpuLocal::new(&[<$N _INIT>]);
        }
    };
}

/// Defines a CPU-local variable.
///
/// # Syntax
///
/// Follow the syntax of a `static` item, but wrap it with `cpu_local!` macro:
///
/// ```
/// cpu_local! {
///     pub static InterruptCounter: usize = 123;
///     ^^^        ^^^^^^^^^^^^^^^^  ^^^^^   ^^^
///     visibility       name        type    initial value
/// }
/// ```
///
/// The initial value must be a constant expression. Thus you can use `const fn`
/// like `RefCell::new(123)`.
#[macro_export]
macro_rules! cpu_local {
    (static $N:ident : $T:ty = $E:expr ;) => {
        $crate::__cpu_local_inner!(, $N, $T, $E);
    };
    (pub static $N:ident : $T:ty = $E:expr ;) => {
        $crate::__cpu_local_inner!(pub, $N, $T, $E);
    };
}

extern "C" {
    static __cpu_local: u8;
    static __cpu_local_end: u8;
}

/// Represents a type that can be used as a CPU-local variable.
///
/// The type must be `Copy` but only in the initialization phase: the value will
/// be copied as-is for each CPU. After the initialization, the value will not
/// be copied anymore and thus the type doesn't have to be `Copy` completely.
///
/// For example, while [`RefCell<T>`] doesn't implement `Copy`, if `T` implements
/// `Copy` it's safe to copy the initial value (`RefCell<T>`) because it's not
/// borrowed yet. This trait is to capture the property.
pub trait CpuLocalable {}
impl CpuLocalable for bool {}
impl CpuLocalable for usize {}
impl<T: Copy + Send> CpuLocalable for RefCell<T> {}

/// A memory space for an initial value of a CPU-local variable.
pub struct InitialValue<T: CpuLocalable + 'static>(T);

impl<T: CpuLocalable + 'static> InitialValue<T> {
    pub const fn new_internal(init: T) -> InitialValue<T> {
        InitialValue(init)
    }
}

/// Safety: It's safe to read the initial value from any CPU.
unsafe impl<T: CpuLocalable + 'static> Sync for InitialValue<T> {}

pub struct CpuLocal<T: CpuLocalable + 'static> {
    init: &'static T,
}

impl<T: CpuLocalable + 'static> CpuLocal<T> {
    pub const fn new(init: &'static InitialValue<T>) -> CpuLocal<T> {
        CpuLocal { init: &init.0 }
    }

    pub fn get(&self) -> &T {
        // TODO: Cache `offset` or ultimately compute it at compile time.
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

impl<T: CpuLocalable + 'static> Deref for CpuLocal<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.get()
    }
}

// Safety: It is safe to access the CPU-local variables from any CPU.
//         Each CPU has its own CPU-local variables.
unsafe impl<T: CpuLocalable + 'static> Sync for CpuLocal<T> {}

/// Initializes the CPU-local variables. This function must be called
/// after the memory allocator is initialized and in each CPU initialization.
pub fn init_percpu() {
    let init_base = unsafe { addr_of!(__cpu_local) as usize };
    let init_end = unsafe { addr_of!(__cpu_local_end) as usize };
    let per_cpu_size = init_end - init_base;

    let percpu_base = PAGE_ALLOCATOR
        .borrow_mut()
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

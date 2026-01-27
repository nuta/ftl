#[repr(C)]
pub struct StartInfo {
    syscall: extern "C" fn(
        a0: usize,
        a1: usize,
        a2: usize,
        a3: usize,
        a4: usize,
        a5: usize,
        n: usize,
    ) -> usize,
}

#[repr(C)]
pub struct StartInfo {
    /// The syscall handler.
    pub syscall: extern "C" fn(
        a0: usize,
        a1: usize,
        a2: usize,
        a3: usize,
        a4: usize,
        a5: usize,
        n: usize,
    ) -> usize,
}

#[repr(C)]
pub struct StartInfo {
    /// The syscall handler.
    pub syscall:
        extern "C" fn(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, n: usize) -> usize,
    /// The minimum page size in bytes. Typically 4096.
    pub min_page_size: usize,
}

pub const PROCESS_NAME_MAX_LEN: usize = 15;

#[repr(C)]
pub struct StartInfo {
    /// The syscall handler.
    pub syscall:
        extern "C" fn(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, n: usize) -> usize,
    /// The minimum page size in bytes. Typically 4096.
    pub min_page_size: usize,
    /// The name of the process.
    pub name: [u8; PROCESS_NAME_MAX_LEN],
    /// The length of `name`.
    pub name_len: u8,
}

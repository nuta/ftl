use crate::handle::HandleId;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct MessageInfo(u32);

#[derive(Debug, Clone, Copy, Hash)]
#[repr(transparent)]
pub struct CallId(u16);

#[derive(Debug)]
pub struct RawMessage {
    pub handles: [HandleId; 2],
    pub buffers: [(usize, usize); 2],
    pub args: [usize; 8],
}

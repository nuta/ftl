use crate::handle::HandleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MessageInfo(u32);

impl MessageInfo {
    pub const fn len(&self) -> usize {
        todo!()
    }

    pub const fn ty(&self) -> u8 {
        todo!()
    }
}

const NUM_HANDLES_MAX: usize = 2;
const NUM_OOLS_MAX: usize = 1;

#[repr(C)]
pub struct OutOfLine {
    pub ptr: usize,
    pub len: usize,
}

#[repr(C)]
pub struct MessageBody {
    pub handles: [HandleId; NUM_HANDLES_MAX],
    pub ools: [OutOfLine; NUM_OOLS_MAX],
    pub inlines: [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TxId(u32);

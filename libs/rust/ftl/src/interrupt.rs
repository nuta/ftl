use ftl_types::error::ErrorCode;

use crate::handle::OwnedHandle;

pub struct Interrupt {
    handle: OwnedHandle,
}

impl Interrupt {
    pub fn new() -> Result<Self, ErrorCode> {
        todo!()
    }
}

impl Handleable for Interrupt {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}

impl fmt::Debug for Interrupt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Interrupt")
            .field(&self.handle.as_usize())
            .finish()
    }
}

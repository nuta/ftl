use ftl_types::handle::HandleId;
use ftl_types::syscall::Syscall;

use crate::arch::syscall1;

pub struct OwnedHandle {
    id: HandleId,
}

impl OwnedHandle {
    pub(crate) const fn new(id: HandleId) -> Self {
        Self { id }
    }

    pub const fn id(&self) -> HandleId {
        self.id
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        let _ = syscall1(Syscall::HandleClose, self.id.as_usize());
    }
}

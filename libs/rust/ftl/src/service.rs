use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::syscall::SYS_SERVICE_REGISTER;

use crate::handle::Handleable;
use crate::handle::OwnedHandle;
use crate::syscall::syscall2;

pub struct Service {
    handle: OwnedHandle,
}

impl Service {
    pub fn register(name: &str) -> Result<Self, ErrorCode> {
        let id = sys_service_register(name)?;
        let handle = OwnedHandle::from_raw(id);
        Ok(Self { handle })
    }
}

impl Handleable for Service {
    fn handle(&self) -> &OwnedHandle {
        &self.handle
    }
}

pub fn sys_service_register(name: &str) -> Result<HandleId, ErrorCode> {
    let id = syscall2(SYS_SERVICE_REGISTER, name.as_ptr() as usize, name.len())?;
    Ok(HandleId::from_raw(id))
}

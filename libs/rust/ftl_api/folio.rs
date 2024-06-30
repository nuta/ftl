use ftl_types::error::FtlError;

use crate::{handle::OwnedHandle, syscall};

pub struct Folio {
    handle: OwnedHandle,
}

impl Folio {
    pub fn from_handle(handle: OwnedHandle) -> Folio {
        Folio { handle }
    }

    pub fn create(len: usize) -> Result<Folio, FtlError> {
        let handle = syscall::folio_create(len)?;
        let owned_handle = OwnedHandle::from_raw(handle);
        Ok(Folio::from_handle(owned_handle))
    }
}

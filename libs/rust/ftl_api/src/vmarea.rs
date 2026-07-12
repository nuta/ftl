use crate::handle::Handle;
use crate::start::start_info;

pub struct VmArea {
    handle: Handle,
}

impl VmArea {
    pub fn allocate(len: usize) -> crate::Result<Self> {
        let start_info = start_info();
        let handle = (start_info.vmarea_allocate)(len)?;
        Ok(Self { handle })
    }

    pub fn write(&self, offset: usize, data: &[u8]) -> crate::Result<()> {
        let start_info = start_info();
        (start_info.vmarea_write)(&self.handle, offset, data)
    }

    pub fn handle(&self) -> &Handle {
        &self.handle
    }
}

impl Drop for VmArea {
    fn drop(&mut self) {
        // TODO: Add vmarea_destroy supercall
    }
}

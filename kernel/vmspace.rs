use alloc::vec::Vec;

use ftl_types::address::PAddr;
use ftl_types::address::VAddr;
use ftl_types::error::FtlError;
use ftl_types::vmspace::PageProtect;

use crate::arch::paddr2vaddr;
use crate::arch::{self};
use crate::folio::Folio;
use crate::handle::Handle;
use crate::spinlock::SpinLock;

struct Mutable {
    folios: Vec<Handle<Folio>>,
    arch: arch::VmSpace,
}

pub struct VmSpace {
    kernel_space: bool,
    mutable: SpinLock<Mutable>,
}

impl VmSpace {
    pub fn kernel_space() -> Result<VmSpace, FtlError> {
        let arch = arch::VmSpace::new()?;
        let mutable = SpinLock::new(Mutable {
            arch,
            folios: Vec::new(),
        });
        Ok(VmSpace {
            kernel_space: true,
            mutable,
        })
    }

    pub fn map_fixed(
        &self,
        vaddr: VAddr,
        paddr: PAddr,
        len: usize,
        _prot: PageProtect,
    ) -> Result<(), FtlError> {
        self.mutable.lock().arch.map(vaddr, paddr, len)?;
        Ok(())
    }

    pub fn map_anywhere(
        &self,
        len: usize,
        folio: Handle<Folio>,
        _prot: PageProtect,
    ) -> Result<VAddr, FtlError> {
        if len != folio.len() {
            return Err(FtlError::InvalidArg);
        }

        if self.kernel_space {
            let vaddr = paddr2vaddr(folio.paddr())?;
            self.mutable.lock().folios.push(folio);
            return Ok(vaddr);
        }

        unimplemented!("userspace support")
    }
}

use ftl_types::error::ErrorCode;

use crate::isolation::INKERNEL_ISOLATION;
use crate::isolation::Isolation;
use crate::shared_ref::RefCounted;
use crate::shared_ref::SharedRef;

pub struct Process {
    isolation: SharedRef<dyn Isolation>,
}

impl Process {
    pub fn new(isolation: SharedRef<dyn Isolation>) -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self { isolation })
    }

    pub fn isolation(&self) -> &SharedRef<dyn Isolation> {
        &self.isolation
    }
}

pub static IDLE_PROCESS: SharedRef<Process> = {
    static INNER: RefCounted<Process> = RefCounted::new(Process {
        isolation: SharedRef::clone_static(&INKERNEL_ISOLATION),
    });
    let process = SharedRef::new_static(&INNER);
    process as SharedRef<Process>
};

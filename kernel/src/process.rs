use alloc::collections::btree_map::BTreeMap;

use ftl_arrayvec::ArrayString;
use ftl_types::environ::PROCESS_NAME_MAX_LEN;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;

use crate::handle::AnyHandle;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::IdleIsolation;
use crate::isolation::InKernelIsolation;
use crate::isolation::Isolation;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::shared_ref::RefCounted;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Thread;
use crate::thread::sys_thread_exit;

pub struct Process {
    name: ArrayString<PROCESS_NAME_MAX_LEN>,
    isolation: SharedRef<dyn Isolation>,
    handle_table: SpinLock<HandleTable>,
}

impl Process {
    pub fn new(
        name: ArrayString<PROCESS_NAME_MAX_LEN>,
        isolation: SharedRef<dyn Isolation>,
    ) -> Result<SharedRef<Self>, ErrorCode> {
        SharedRef::new(Self {
            name,
            isolation,
            handle_table: SpinLock::new(HandleTable::new()),
        })
    }

    pub fn new_inkernel(
        name: ArrayString<PROCESS_NAME_MAX_LEN>,
    ) -> Result<SharedRef<Self>, ErrorCode> {
        let isolation = InKernelIsolation::new()?;
        Self::new(name, isolation)
    }

    #[allow(unused)] // For debugging
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn isolation(&self) -> &SharedRef<dyn Isolation> {
        &self.isolation
    }

    pub fn handle_table(&self) -> &SpinLock<HandleTable> {
        &self.handle_table
    }
}

const NUM_HANDLES_MAX: usize = 1024;

pub struct HandleTable {
    handles: BTreeMap<usize, AnyHandle>,
    next_id: usize,
}

impl HandleTable {
    pub const fn new() -> Self {
        Self {
            handles: BTreeMap::new(),
            next_id: 1,
        }
    }

    pub fn insert<H: Into<AnyHandle>>(&mut self, object: H) -> Result<HandleId, ErrorCode> {
        if self.handles.len() >= NUM_HANDLES_MAX {
            return Err(ErrorCode::TooManyHandles);
        }

        let id = HandleId::from_raw(self.next_id);
        self.next_id += 1;
        self.handles.insert(id.as_usize(), object.into());
        Ok(id)
    }

    // TODO: Can we return a reference instead of a clone?
    pub fn get_any(&self, id: HandleId) -> Option<AnyHandle> {
        self.handles.get(&id.as_usize()).cloned()
    }

    pub fn get<T: Handleable>(&self, id: HandleId) -> Result<Handle<T>, ErrorCode> {
        self.get_any(id)
            .ok_or(ErrorCode::HandleNotFound)?
            .downcast::<T>()
            .ok_or(ErrorCode::InvalidHandle)
    }

    pub fn remove(&mut self, id: HandleId) -> Result<AnyHandle, ErrorCode> {
        self.handles
            .remove(&id.as_usize())
            .ok_or(ErrorCode::HandleNotFound)
    }

    pub fn clear(&mut self) {
        for handle in self.handles.values() {
            handle.bypass_check().close();
        }
        self.handles.clear();
    }
}

impl Handleable for Process {}

pub fn sys_process_create_inkernel(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let name_slice = UserSlice::new(UserPtr::new(a0), a1)?;

    let process = current.process();

    let mut name_buf = [0; PROCESS_NAME_MAX_LEN];
    process.isolation().read_bytes(&name_slice, &mut name_buf)?;
    let name = ArrayString::from_ascii_str(&name_buf).map_err(|_| ErrorCode::InvalidArgument)?;

    let new_process = Process::new_inkernel(name)?;
    let id = process
        .handle_table()
        .lock()
        .insert(Handle::new(new_process, HandleRight::ALL))?;

    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_process_exit(current: &SharedRef<Thread>) -> Result<SyscallResult, ErrorCode> {
    current.process().handle_table().lock().clear();
    sys_thread_exit(current)
}

pub static IDLE_PROCESS: SharedRef<Process> = {
    static ISOLATION_INNER: RefCounted<IdleIsolation> =
        RefCounted::new_static(IdleIsolation::new());

    static INNER: RefCounted<Process> = RefCounted::new_static(Process {
        name: ArrayString::from_static("[idle]"),
        isolation: SharedRef::new_static(&ISOLATION_INNER) as SharedRef<dyn Isolation>,
        handle_table: SpinLock::new(HandleTable::new()),
    });
    let process = SharedRef::new_static(&INNER);
    process as SharedRef<Process>
};

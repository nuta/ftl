use alloc::collections::btree_map::BTreeMap;

use ftl_arrayvec::ArrayString;
use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;
use ftl_types::start_info::PROCESS_NAME_MAX_LEN;

use crate::handle::AnyHandle;
use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::IdleIsolation;
use crate::isolation::InKernelIsolation;
use crate::isolation::Isolation;
use crate::isolation::SandboxIsolation;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::shared_ref::RefCounted;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::syscall::SyscallResult;
use crate::thread::Thread;
use crate::thread::sys_thread_exit;
use crate::vmspace::VmSpace;

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

pub struct ReservedSlot<'a, const N: usize>(&'a mut HandleTable);

impl<'a, const N: usize> ReservedSlot<'a, N> {
    pub fn new(table: &'a mut HandleTable) -> Self {
        Self(table)
    }
}

impl<'a> ReservedSlot<'a, 1> {
    pub fn insert<H: Into<AnyHandle>>(self, object: H) -> HandleId {
        self.0.do_insert(object)
    }
}

impl<'a> ReservedSlot<'a, 2> {
    pub fn insert2<H: Into<AnyHandle>>(self, object0: H, object1: H) -> (HandleId, HandleId) {
        let id0 = self.0.do_insert(object0);
        let id1 = self.0.do_insert(object1);
        (id0, id1)
    }
}

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

    fn do_insert<H: Into<AnyHandle>>(&mut self, object: H) -> HandleId {
        let id = HandleId::from_raw(self.next_id);
        self.next_id += 1;
        self.handles.insert(id.as_usize(), object.into());
        id
    }

    pub fn reserve<const N: usize>(&mut self) -> Result<ReservedSlot<'_, N>, ErrorCode> {
        if self.handles.len() + N > NUM_HANDLES_MAX {
            return Err(ErrorCode::TooManyHandles);
        }

        Ok(ReservedSlot::new(self))
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

fn read_process_name(
    current: &SharedRef<Thread>,
    name_slice: &UserSlice,
) -> Result<ArrayString<PROCESS_NAME_MAX_LEN>, ErrorCode> {
    if name_slice.len() == 0 || name_slice.len() > PROCESS_NAME_MAX_LEN {
        return Err(ErrorCode::InvalidArgument);
    }

    let mut name_buf = [0; PROCESS_NAME_MAX_LEN];
    current
        .process()
        .isolation()
        .read_bytes(name_slice, &mut name_buf[..name_slice.len()])?;

    ArrayString::from_ascii_str(&name_buf[..name_slice.len()])
        .map_err(|_| ErrorCode::InvalidArgument)
}

pub fn sys_process_create_sandboxed(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
) -> Result<SyscallResult, ErrorCode> {
    let vmspace_id = HandleId::from_raw(a0);
    let name_slice = UserSlice::new(UserPtr::new(a1), a2)?;
    let name = read_process_name(current, &name_slice)?;

    let mut handle_table = current.process().handle_table().lock();
    let vmspace = handle_table
        .get::<VmSpace>(vmspace_id)?
        .authorize(HandleRight::WRITE)?;

    let slot = handle_table.reserve()?;

    let isolation = SandboxIsolation::new(vmspace)?;
    let new_process = Process::new(name, isolation)?;

    let handle = Handle::new(new_process, HandleRight::ALL);
    let id = slot.insert(handle);
    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_process_create_inkernel(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
    a2: usize,
) -> Result<SyscallResult, ErrorCode> {
    if !current.process().isolation().is_inkernel() {
        return Err(ErrorCode::NotAllowed);
    }

    let vmspace_id = HandleId::from_raw(a0);
    let name_slice = UserSlice::new(UserPtr::new(a1), a2)?;
    let name = read_process_name(current, &name_slice)?;

    let mut handle_table = current.process().handle_table().lock();
    let vmspace = handle_table
        .get::<VmSpace>(vmspace_id)?
        .authorize(HandleRight::WRITE)?;

    let slot = handle_table.reserve()?;

    let isolation = InKernelIsolation::new(vmspace)?;
    let new_process = Process::new(name, isolation)?;

    let handle = Handle::new(new_process, HandleRight::ALL);
    let id = slot.insert(handle);
    Ok(SyscallResult::Return(id.as_usize()))
}

pub fn sys_process_exit(current: &SharedRef<Thread>) -> Result<SyscallResult, ErrorCode> {
    current.process().handle_table().lock().clear();
    sys_thread_exit(current)
}

pub fn sys_process_inject_handle(
    current: &SharedRef<Thread>,
    a0: usize,
    a1: usize,
) -> Result<SyscallResult, ErrorCode> {
    let process_id = HandleId::from_raw(a0);
    let handle_id = HandleId::from_raw(a1);

    let current_process = current.process();
    let mut current_table = current_process.handle_table().lock();
    let target_process = current_table
        .get::<Process>(process_id)?
        .authorize(HandleRight::WRITE)?;

    if SharedRef::eq(&target_process, &current_process) {
        // Cannot inject handle into the current process.
        return Err(ErrorCode::InvalidArgument);
    }

    let handle = current_table.remove(handle_id)?;

    drop(current_table);
    let mut target_table = target_process.handle_table().lock();
    let slot = target_table.reserve()?;

    // TODO: What if the target table is full? Should we roll back?
    let id = slot.insert(handle);
    Ok(SyscallResult::Return(id.as_usize()))
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

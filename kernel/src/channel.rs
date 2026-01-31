use ftl_types::error::ErrorCode;
use ftl_types::handle::HandleId;

use crate::handle::Handle;
use crate::handle::HandleRight;
use crate::handle::Handleable;
use crate::isolation::UserPtr;
use crate::isolation::UserSlice;
use crate::shared_ref::SharedRef;
use crate::spinlock::SpinLock;
use crate::thread::Thread;

struct Mutable {
    peer: Option<SharedRef<Channel>>,
}

pub struct Channel {
    mutable: SpinLock<Mutable>,
}

impl Channel {
    pub fn new() -> Result<(SharedRef<Self>, SharedRef<Self>), ErrorCode> {
        let ch0 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable { peer: None }),
        })?;
        let ch1 = SharedRef::new(Self {
            mutable: SpinLock::new(Mutable {
                peer: Some(ch0.clone()),
            }),
        })?;
        ch0.mutable.lock().peer = Some(ch1.clone());

        Ok((ch0, ch1))
    }
}

impl Handleable for Channel {}

pub fn sys_channel_create(current: &SharedRef<Thread>, a0: usize) -> Result<usize, ErrorCode> {
    let ids = UserSlice::new(UserPtr::new(a0), size_of::<[HandleId; 2]>())?;

    let (ch0, ch1) = Channel::new()?;
    let handle0 = Handle::new(ch0, HandleRight::ALL);
    let handle1 = Handle::new(ch1, HandleRight::ALL);

    let process = current.process();
    let mut handle_table = process.handle_table().lock();
    let id0 = handle_table.insert(handle0)?;
    let id1 = handle_table.insert(handle1)?;

    let isolation = process.isolation();
    crate::isolation::write(isolation, ids, 0, [id0, id1])?;

    Ok(0)
}

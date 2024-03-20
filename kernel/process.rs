use alloc::vec::Vec;

use crate::handle::Handle;
use crate::handle::HandleTable;
use crate::handle::Handleable;
use crate::spinlock::SpinLock;
use crate::thread::Thread;

pub struct Process {
    handles: SpinLock<HandleTable>,
    threads: Vec<Handle<Thread>>,
}

impl Handleable for Process {}

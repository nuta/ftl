use alloc::sync::Arc;

use ftl_utils::fxhash::FxHashMap;

use crate::process::PId;
use crate::process::Process;
use crate::types::c_int;
use crate::types::errno::Errno;

pub struct PIdTable {
    pids: FxHashMap<c_int, Arc<Process>>,
    next: c_int,
    max: c_int,
}

impl PIdTable {
    pub fn new() -> Self {
        Self {
            pids: FxHashMap::new(),
            next: 2,   // skip init process (PID=1) which always exists
            max: 1024, // TODO: make this configurable
        }
    }

    pub fn insert(&mut self, pid: PId, process: Arc<Process>) {
        if self.pids.insert(pid.as_int(), process).is_some() {
            unreachable!("PID {} already exists", pid);
        }
    }

    pub fn get(&self, pid: PId) -> Option<Arc<Process>> {
        self.pids.get(&pid.as_int()).cloned()
    }

    pub fn allocate(&mut self) -> Result<PId, Errno> {
        let mut pid = self.next;
        for _ in 0..self.max {
            if !self.pids.contains_key(&pid) {
                return Ok(PId::new(pid));
            }

            if pid == self.max {
                pid = 2;
            } else {
                pid += 1;
            }
        }

        Err(Errno::EAGAIN) // Too many processes
    }

    pub fn remove(&mut self, pid: PId) {
        self.pids.remove(&pid.as_int());
    }
}

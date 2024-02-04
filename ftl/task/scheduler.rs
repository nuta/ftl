use alloc::{collections::VecDeque, sync::Arc};

use crate::sync::mutex::Mutex;

use super::fiber::RawFiber;

pub struct Scheduler {
    run_queue: VecDeque<Arc<Mutex<RawFiber>>>,
}

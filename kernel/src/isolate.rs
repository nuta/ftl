use ftl_types::handle::HandleRight;

use crate::shared_ref::Handleable;

pub struct Isolate {}

impl Isolate {
    pub fn new() -> Self {
        Self {}
    }
}

impl Handleable for Isolate {
    const DEFAULT_RIGHT: HandleRight = HandleRight::WRITE;
}

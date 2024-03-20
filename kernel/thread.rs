use crate::arch;
use crate::handle::Handle;
use crate::handle::Handleable;
use crate::process::Process;

pub struct Thread {
    process: Handle<Process>,
    context: arch::Context,
}

impl Handleable for Thread {}

pub struct Scheduler {
    threads: Vec<SharedRef<Thread>>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            threads: Vec::new(),
        }
    }
}

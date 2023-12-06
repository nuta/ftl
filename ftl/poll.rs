use crate::Handle;

pub struct Ready(u32);

impl Ready {
    pub const READABLE: Ready = Ready(0b0001);
    pub const WRITABLE: Ready = Ready(0b0010);
}

pub struct Interest(u32);

impl Interest {
    pub const READABLE: Interest = Interest(0b0001);
    pub const WRITABLE: Interest = Interest(0b0010);
}

struct Event {}

struct Poll {}

impl Poll {
    pub fn register(&mut self, handle: Handle, interest: Interest) -> crate::Result<()> {
        todo!()
    }

    /// Blocks until an event is ready.
    pub fn poll(&mut self) -> crate::Result<Event> {
        todo!()
    }
}

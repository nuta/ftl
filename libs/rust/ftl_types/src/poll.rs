use crate::handle::HandleId;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum EventKind {
    PollNotified = 1,
}

#[derive(Debug)]
#[repr(C)]
pub struct Event(u32);

impl Event {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn new(kind: EventKind, handle_id: HandleId) -> Self {
        // FIXME: make handle_id u32
        let raw = handle_id.as_usize() as u32 | (kind as u32) << 24;
        Self(raw)
    }

    pub const fn as_raw(&self) -> usize {
        self.0 as usize
    }

    pub fn handle_id(&self) -> HandleId {
        HandleId::new(self.0 as usize & 0x00ff_ffff)
    }

    pub fn kind(&self) -> EventKind {
        let kind_id = self.0 as usize >> 24;
        match kind_id {
            1 => EventKind::PollNotified,
            _ => panic!("invalid event kind: {}", kind_id),
        }
    }
}

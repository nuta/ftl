#[derive(Debug)]
pub enum FtlError {}

pub type Result<T> = std::result::Result<T, FtlError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(u32);

#[derive(Debug)]
pub struct Event {
    pub handle: Handle,
    pub ready: Ready,
}

#[derive(Debug)]
pub struct Ready {
    pub readable: bool,
    pub writable: bool,
}

pub struct EventQueue {}

impl EventQueue {
    pub fn new() -> Self {
        Self {}
    }

    pub fn next(&mut self) -> Result<Event> {
        todo!()
    }
}

pub struct Channel {
    //
}

pub struct DuplexChannel {
    pub rx: Channel,
    pub tx: Channel,
}

pub struct BufChain {
    chain: Vec<u8>,
}

pub struct Environ {}

pub struct Badge {}
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        println!($($arg)*);
    };
}

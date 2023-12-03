use core::cell::UnsafeCell;

use alloc::collections::VecDeque;

pub struct Message {}

/// A single-producer, single-consumer FIFO queue.
struct Ring {
    messages: VecDeque<Message>,
}

/// A single-producer, single-consumer bidirectional message queue.
pub struct Channel {
    tx: *mut Ring,
    rx: *const Ring,
}

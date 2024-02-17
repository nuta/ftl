#![no_std]

extern crate alloc;

pub mod address;
pub mod environ;
pub mod error;
pub mod handle;
pub mod message;
pub mod signal;
pub mod spec;

// FIXME: IDL
#[derive(Debug)]
pub enum Message {
    Ping(usize),
    Pong(usize),
}

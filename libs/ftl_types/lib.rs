#![no_std]

extern crate alloc;

pub mod environ;
pub mod error;
pub mod handle;
pub mod spec;
pub mod address;

// FIXME: IDL
#[derive(Debug)]
pub enum Message {
    Ping(usize),
    Pong(usize),
}

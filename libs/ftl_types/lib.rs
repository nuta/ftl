#![no_std]

pub mod error;
pub mod handle;
pub mod spec;

// FIXME: IDL
#[derive(Debug)]
pub enum Message {
    Ping(usize),
    Pong(usize),
}

#![no_std]

pub mod error;
pub mod handle;

// FIXME: IDL
#[derive(Debug)]
pub enum Message {
    Ping(usize),
    Pong(usize),
}

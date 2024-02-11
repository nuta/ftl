#![no_std]

pub mod fibers;
pub const FIBER_INITS: &[fn()] = &[ping::main, pong::main];

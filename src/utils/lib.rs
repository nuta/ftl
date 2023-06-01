//! A self-contained utilities for both the kernel and user programs.
#![no_std]
// TODO: testing features
#![feature(const_mut_refs)]
#![feature(const_pin)]

#[macro_use]
pub mod macros;

pub mod alignment;
pub mod linked_list;

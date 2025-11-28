#![allow(clippy::arithmetic_side_effects)]
#![deny(missing_docs)]
//#![cfg_attr(not(test), no_std)
#![no_std]

//! A lending program for the casper blockchain.

//pub mod entrypoint;
pub mod error;
//pub mod instruction;
pub mod math;
pub mod processor;
pub mod pyth;
pub mod state;

// Export current sdk types for downstream users building with a different sdk
// version



extern crate alloc; 
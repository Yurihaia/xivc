//! The core of the XIVC project.
//!
//! This crate contains the essential types that every
//! XIVC simulation will need to interact with.
//!
//! TODO write more crate documentation

#![cfg_attr(not(test), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod enums;
pub mod job;
pub mod math;
pub mod timing;
pub mod util;
pub mod world;

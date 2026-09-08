//! Shark Books Community product core entry point from SBC-3 onward.
//! The frozen SBC-2 domain implementation remains in `lib.rs`; this entry point
//! re-exports it and adds the bounded bank-import module without rewriting the
//! already-proven SBC-2 source.

#![forbid(unsafe_code)]

#[path = "lib.rs"]
mod domain;

pub use domain::*;
pub mod bank_import;

//! Compile/test harness for the dependency-free SBC-7B2 Action System Phase A.
//! The production module remains `src/action_system.rs`; this wrapper adds no runtime surface.

pub use shark_foundation::{FoundationError, FoundationErrorCode, FoundationResult};

#[path = "../src/action_system.rs"]
mod action_system;

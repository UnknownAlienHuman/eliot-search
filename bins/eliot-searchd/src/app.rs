//! Command application for the ELIOT Search daemon binary.
//!
//! The public binary entry remains stable here. Command parsing, stdio
//! protocol handling, DIRECT command orchestration and regression tests are
//! isolated behind one private owner pending responsibility-level extraction.

#[path = "app/kernel.rs"]
mod kernel;

pub use kernel::*;

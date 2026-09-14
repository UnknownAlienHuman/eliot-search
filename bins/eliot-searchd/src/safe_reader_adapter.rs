//! Final-handle platform adapter proving containment before shared-kernel reads.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! token/root containment, native identity, final-handle backend execution,
//! whole-file translation and regression coverage.

#[path = "safe_reader_adapter/kernel.rs"]
mod kernel;

pub use kernel::*;

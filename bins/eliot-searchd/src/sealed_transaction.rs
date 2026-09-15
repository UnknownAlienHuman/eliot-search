//! Idempotent intent/readback/reconciliation around DPAPI sealed objects.
//!
//! The stable surface delegates to bounded private owners for closed failures,
//! durable transaction models, exact Windows metadata coding, lock/file I/O,
//! put reconciliation and read-only status inspection.

#[path = "sealed_transaction/kernel.rs"]
mod kernel;

pub use kernel::*;

//! Effective configuration composition and truthful capability readiness.
//!
//! This stable facade preserves every existing daemon call path while the
//! oversized composition kernel is split by responsibility. The kernel remains
//! the sole implementation owner during the staged refactor; no second config
//! state machine or alternate precedence path is introduced.

mod kernel;
pub use kernel::*;

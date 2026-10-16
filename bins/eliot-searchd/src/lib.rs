//! Internal native adapters shared by this package's binary and test targets.
//!
//! This library is not a provider API or a mutable capability owner. Its safe
//! boundary lets the harness-only legacy targets retain `forbid(unsafe_code)`.

#![deny(unsafe_code)]
#![deny(missing_docs)]

/// Closed, bounded, redacted diagnostics (T40; shared by the daemon binary
/// and its process tests so budgets and sentinels have one owner).
pub mod diagnostics;
#[doc(hidden)]
pub mod native_file;
/// Measured resource budgets on a frozen corpus (T40; same single owner).
pub mod resource_budgets;

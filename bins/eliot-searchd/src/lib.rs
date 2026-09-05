//! Internal native adapters shared by this package's binary and test targets.
//!
//! This library is not a provider API or a mutable capability owner. Its safe
//! boundary lets the harness-only legacy targets retain `forbid(unsafe_code)`.

#![deny(unsafe_code)]
#![deny(missing_docs)]

#[doc(hidden)]
pub mod native_file;

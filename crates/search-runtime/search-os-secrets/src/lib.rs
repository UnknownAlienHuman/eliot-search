//! Opaque secret lifecycle plus pure compatibility contracts for protected data.
//!
//! [`lifecycle`] owns finite encrypted-record state and short-lived plaintext
//! leases. [`legacy_revision_protection`] owns only the frozen byte layout and
//! binding validation for the legacy DIRECT revision envelope. Neither module
//! performs platform I/O; qualified adapters remain separate.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod legacy_revision_protection;
mod lifecycle;

pub use legacy_revision_protection::*;
pub use lifecycle::*;

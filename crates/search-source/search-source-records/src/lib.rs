//! Source-owned value/schema boundary, without I/O or live admission authority.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// Preserve the existing registry's allowances for the relocated frozen modules.
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

pub mod batch;
pub mod error;
pub mod legacy_direct;
pub mod legacy_root_catalog;
pub mod membership;
pub mod namespace;
pub mod operation;
pub mod portfolio;
pub mod portfolio_history;
pub mod provenance;
pub mod root;
pub mod root_currentness;
pub mod snapshot;
pub mod source;

#[cfg(test)]
#[path = "portfolio_history_tests.rs"]
mod portfolio_history_tests;

#[cfg(test)]
#[path = "provenance_tests.rs"]
mod provenance_tests;

//! Immutable source occurrences and exact historical replay/readback semantics.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// The relocated frozen occurrence implementation retains its existing allowances.
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

pub use search_source_records::error;
pub mod registry_restore;
pub mod revision;

#[cfg(test)]
mod revision_tests;

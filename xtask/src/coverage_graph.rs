//! Deterministic coverage-graph helper primitives.
//!
//! These IO-free functions preserve the historical byte contracts used by the
//! coverage registries. Graph reconciliation is Rust-owned by
//! [`crate::coverage_graph_generation`]; route assignment remains an explicit
//! reviewed-registry operation rather than a lexical heuristic.

mod digest;
mod markdown;
mod text;

pub use digest::digest_text;
pub use markdown::{Heading, heading_rows};
pub use text::{
    CoverageGraphError, arr, module_refs_to_packages, quote_json, replace_once,
    slug, words,
};

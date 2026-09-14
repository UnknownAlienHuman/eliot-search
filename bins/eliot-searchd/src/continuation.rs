//! Bounded live-authorized continuation windows for DIRECT search.
//!
//! The stable daemon-local surface is exported here; entropy acquisition,
//! finite window state, source-fence revalidation and the regression corpus
//! remain behind one private owner pending responsibility-level extraction.

#[path = "continuation/kernel.rs"]
mod kernel;

pub use kernel::*;

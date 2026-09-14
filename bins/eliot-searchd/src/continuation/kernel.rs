//! Bounded live-authorized continuation windows for DIRECT search.
//!
//! Tokens remain opaque session-local locators. Qualified entropy, closed
//! model/error vocabulary, finite window state and regression coverage have
//! separate private owners behind the existing continuation surface.

mod catalog;
mod entropy;
mod model;
mod spec;

pub use catalog::*;
pub use entropy::*;
pub use model::*;
pub use spec::*;

#[cfg(test)]
mod tests;

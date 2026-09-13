//! Legacy DIRECT preparation framing and representation identity.
//!
//! Persisted algorithm tags, frame codec and representation preimage have
//! separate private owners behind this stable package-internal facade.

mod algorithm;
mod frame;
mod identity;

pub use algorithm::*;
pub use frame::*;
pub use identity::*;

#[cfg(test)]
mod tests;

//! Pairing-secret composition behind the stable daemon-local facade.

#![allow(dead_code)]

mod binding;
mod composer;
mod receipts;
mod revocation;
mod rotation;
mod spec;
mod vault;

pub use binding::*;
pub use composer::*;
pub use receipts::*;
pub use spec::*;
pub use vault::*;

#[cfg(test)]
mod tests;

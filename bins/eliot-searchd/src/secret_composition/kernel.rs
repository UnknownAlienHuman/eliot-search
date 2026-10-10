//! Pairing-secret composition behind the stable daemon-local facade.

#![allow(dead_code)]

#[path = "kernel/binding.rs"]
mod binding;
#[path = "kernel/composer.rs"]
mod composer;
#[path = "kernel/receipts.rs"]
mod receipts;
#[path = "kernel/revocation.rs"]
mod revocation;
#[path = "kernel/rotation.rs"]
mod rotation;
#[path = "kernel/spec.rs"]
mod spec;
#[path = "kernel/vault.rs"]
mod vault;

pub use binding::*;
pub use composer::*;
pub use receipts::*;
pub use spec::*;
pub use vault::*;

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;

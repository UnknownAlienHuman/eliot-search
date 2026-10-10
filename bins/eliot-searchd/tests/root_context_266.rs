//! Focused proofs over the actual private daemon command-context implementation.
//! This target does not compile the unrelated historical binary test harnesses.

#[allow(dead_code)]
#[path = "../src/qualified_entropy.rs"]
mod qualified_entropy;

#[allow(dead_code)]
#[path = "../src/owner_composition/kernel/operation.rs"]
mod operation;

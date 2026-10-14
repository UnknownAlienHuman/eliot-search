//! DIRECT ingestion composition behind the plaintext store.

#[path = "kernel/batch.rs"]
mod batch;
#[path = "kernel/entry.rs"]
mod entry;
#[path = "kernel/plan.rs"]
mod plan;
#[path = "kernel/policy.rs"]
mod policy;
#[path = "kernel/spec.rs"]
mod spec;

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;

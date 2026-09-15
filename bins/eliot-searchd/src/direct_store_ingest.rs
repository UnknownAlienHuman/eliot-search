//! Bounded DIRECT ingestion with an explicit revision-storage publication barrier.
//!
//! The stable plaintext-catalog surface delegates to bounded private owners
//! for source entrypoints, one coherent admission/registry view, snapshot
//! planning, all-before-write batch execution and the ingestion regression
//! corpus. Denied or ambiguous sources remain unable to reach CAS.

#[path = "direct_store_ingest/kernel.rs"]
mod kernel;

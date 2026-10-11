//! Pure v2 admission receipt issuance and current-policy verification.
//!
//! The source-admission owner supplies immutable policy, observation and decision
//! models. This leaf owns the complete receipt behavior and private provenance;
//! it performs no filesystem, network, process, clock or durable-state I/O.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Bounded immutable receipts over producer-derived admission v2 decisions.
pub mod receipt_v2;

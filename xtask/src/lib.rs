//! Bounded Rust validation tooling (T41).
//!
//! Small explicit Cargo command surface for registry and evidence checks.
//! This crate is not a swarm controller and never issues control records.

pub mod accepted_evidence;
pub mod compute_accepted_evidence;
pub mod context_artifact;
pub mod coverage_graph;
pub mod package_maps;
pub mod ticket_drafts;
pub mod ticket_planner;
pub mod validate_accepted_evidence;

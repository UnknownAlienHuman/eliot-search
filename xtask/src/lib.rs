//! Bounded Rust validation tooling (T41).
//!
//! Small explicit Cargo command surface for registry and evidence checks.
//! This crate is not a swarm controller and never issues control records.

pub mod accepted_evidence;
pub mod agent_drafts;
pub mod architecture_coverage;
pub mod architecture_coverage_contracts;
pub mod canonical_digest_guard;
mod cargo_metadata_adapter;
pub mod compute_accepted_evidence;
pub mod coverage_graph;
pub mod coverage_graph_generation;
pub mod coverage_graph_validation;
pub mod impl_program;
pub mod integration_bootstrap;
pub mod milestone_packets;
pub mod p00_acceptance;
pub mod package_maps;
mod package_status;
pub mod qdrant_boundary;
pub mod product_git_helper;
pub mod validate_accepted_evidence;

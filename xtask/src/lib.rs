//! Bounded Rust validation tooling (T41).
//!
//! Small explicit Cargo command surface for registry and evidence checks.
//! This crate is not a swarm controller and never issues control records.

pub mod accepted_evidence;
pub mod agent_drafts;
pub mod architecture_coverage;
pub mod architecture_coverage_contracts;
pub mod compute_accepted_evidence;
pub mod context_artifact;
pub mod context_artifact_builder;
pub mod context_artifact_io;
pub mod context_artifact_validation;
pub mod context_materialization;
pub mod context_materialization_builder;
pub mod context_materialization_validation;
pub mod coverage_graph;
pub mod git_tree;
pub mod impl_program;
pub mod integration_bootstrap;
pub mod milestone_packets;
pub mod p00_acceptance;
pub mod package_maps;
pub mod qdrant_boundary;
pub mod ticket_drafts;
pub mod ticket_issuance_validation;
pub mod ticket_planner;
pub mod validate_accepted_evidence;

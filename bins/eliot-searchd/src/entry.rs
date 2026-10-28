//! Current daemon command routing.
//!
//! Legacy interceptors remain until one connected application replaces them.
//! Shell help is emitted by the application, not appended by this entrypoint.

#![deny(unsafe_code)]

#[cfg(feature = "wave4-query")]
#[allow(dead_code)]
mod access_composition;
mod app;
mod source_classification;
mod catalog_presence;
mod catalog_quarantine;
mod config_composition;
mod control_identity;
mod continuation;
mod development;
mod direct_preparation;
#[path = "secure_direct_store.rs"]
mod direct_store;
mod directory_manifest;
#[allow(dead_code)]
#[path = "source_composition.rs"]
mod git_source_composition;
mod maintenance;
#[cfg(test)]
mod maintenance_guard;
mod native_root_command;
pub(crate) mod owner_composition;
#[path = "direct_store.rs"]
mod plaintext_direct_store;
mod preparation_composition;
mod protocol_io;
mod provider_composition;
mod public_runtime_service;
#[allow(dead_code)]
mod publication_composition;
mod qualified_entropy;
mod registry_control;
#[cfg(feature = "wave4-query")]
#[allow(dead_code)]
mod query_composition;
#[cfg(feature = "wave4-query")]
#[path = "query_composition/live_authority.rs"]
mod query_live_authority;
#[cfg(feature = "wave4-query")]
#[path = "query_composition/serving.rs"]
mod query_serving_composition;
#[cfg(feature = "wave3-index")]
#[allow(dead_code)]
mod rebuild_composition;
mod result_handles;
mod revision_protection;
mod safe_reader_adapter;
#[allow(dead_code)]
mod sealed_digest;
#[allow(dead_code)]
pub(crate) mod sealed_owner_epoch;
#[allow(dead_code)]
pub(crate) mod sealed_root_identity;
#[allow(dead_code)]
pub(crate) mod sealed_root_lock;
#[allow(dead_code)]
mod sealed_store;
#[allow(dead_code)]
mod sealed_transaction;
#[allow(dead_code)]
mod sealed_transaction_guard;
mod secret_composition;
mod secure_commands;
mod service_output;
mod sha256;
#[path = "direct_store/composition.rs"]
mod source_composition;
mod source_fence;
mod source_migration_command;
mod source_roots;
mod storage_security;

#[cfg(all(test, windows))]
mod protected_ingest_tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    secure_commands::maybe_refuse_legacy_effects()
        .or_else(owner_composition::maybe_run_direct_registry)
        .or_else(native_root_command::maybe_run)
        .or_else(source_migration_command::maybe_run)
        .or_else(preparation_composition::maybe_run)
        .or_else(public_runtime_service::maybe_run)
        .or_else(secure_commands::maybe_run)
        .unwrap_or_else(app::run_main)
}

//! Primary ELIOT Search daemon entrypoint.
//!
//! The primary binary composes the owner-fenced persistent DIRECT runtime,
//! bounded continuation windows, opaque source handles, directory manifests,
//! guarded maintenance, authenticated loopback access, and platform revision
//! protection. The earlier immutable snapshot/BM25 daemon remains
//! `eliot-search-snapshotd`.

#![deny(unsafe_code)]

mod app;
mod authenticated_proxy;
mod continuation;
mod development;
#[path = "direct_store.rs"]
mod plaintext_direct_store;
#[path = "secure_direct_store.rs"]
mod direct_store;
mod directory_manifest;
mod endpoint;
mod maintenance;
mod maintenance_guard;
mod public_runtime_service;
mod result_handles;
mod revision_protection;
mod service_output;
mod sha256;
mod source_fence;

use std::process::ExitCode;

fn main() -> ExitCode {
    authenticated_proxy::maybe_run()
        .or_else(public_runtime_service::maybe_run)
        .unwrap_or_else(app::run_main)
}

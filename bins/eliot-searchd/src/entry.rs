//! Primary ELIOT Search daemon entrypoint.
//!
//! The primary binary now composes the owner-fenced persistent DIRECT runtime,
//! bounded continuation windows, opaque source handles, directory manifests,
//! guarded maintenance, and the one-shot command surface. The earlier immutable
//! snapshot/BM25 daemon remains available as `eliot-search-snapshotd`.

#![forbid(unsafe_code)]

mod app;
mod continuation;
mod development;
mod direct_store;
mod directory_manifest;
mod maintenance;
mod maintenance_guard;
mod public_runtime_service;
mod result_handles;
mod service_output;
mod sha256;
mod source_fence;

use std::process::ExitCode;

fn main() -> ExitCode {
    public_runtime_service::maybe_run().unwrap_or_else(app::run_main)
}

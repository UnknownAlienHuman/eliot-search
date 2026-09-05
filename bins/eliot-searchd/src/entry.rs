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
mod catalog_presence;
mod continuation;
mod development;
mod direct_preparation;
#[path = "direct_store.rs"]
mod plaintext_direct_store;
#[path = "secure_direct_store.rs"]
mod direct_store;
mod directory_manifest;
mod endpoint;
mod maintenance;
mod maintenance_guard;
mod protocol_io;
mod public_runtime_service;
mod result_handles;
mod revision_protection;
mod secure_commands;
mod service_output;
mod sha256;
mod source_fence;
mod source_root_commands;
mod source_roots;
mod storage_security;

#[cfg(all(test, windows))]
mod protected_ingest_tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    let result = authenticated_proxy::maybe_run()
        .or_else(public_runtime_service::maybe_run)
        .or_else(secure_commands::maybe_run)
        .unwrap_or_else(app::run_main);
    // The secure dispatcher owns the existing public help. Append the new
    // composition commands there too, rather than updating only legacy help.
    if result == ExitCode::SUCCESS
        && std::env::args_os().len() == 2
        && std::env::args_os()
            .nth(1)
            .is_some_and(|argument| argument == "--help" || argument == "-h")
    {
        print!(
            "{}",
            concat!(
                "\nPERSISTENT SOURCE-ROOT REGISTRATION:\n",
                "  eliot-searchd --source-roots ROOT\n",
                "  eliot-searchd --register-source-root ROOT DIRECTORY\n",
                "  eliot-searchd --unregister-source-root ROOT DIRECTORY\n",
                "  eliot-searchd --sync-source-roots ROOT\n",
                "Registration controls explicit observation, not access grants or purge.\n",
                "Unregistering does not revoke already retained revisions.\n"
            )
        );
    }
    result
}

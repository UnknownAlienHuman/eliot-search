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
mod catalog_quarantine;
mod continuation;
mod development;
mod direct_preparation;
#[path = "secure_direct_store.rs"]
mod direct_store;
mod directory_manifest;
mod endpoint;
mod maintenance;
mod maintenance_guard;
#[path = "direct_store.rs"]
mod plaintext_direct_store;
mod preparation_composition;
mod protocol_io;
mod public_runtime_service;
mod result_handles;
mod revision_protection;
mod safe_reader_adapter;
mod secure_commands;
mod service_output;
mod sha256;
mod source_fence;
mod source_migration_command;
mod source_root_commands;
mod source_roots;
mod storage_security;

#[cfg(all(test, windows))]
mod protected_ingest_tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    let result = source_migration_command::maybe_run()
        .or_else(preparation_composition::maybe_run)
        .or_else(authenticated_proxy::maybe_run)
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
                "Unregistering does not revoke already retained revisions.\n",
                "\nRETAINED REVISION PREPARATION:\n",
                "  eliot-searchd --prepare-revision ROOT REVISION_ID\n",
                "  eliot-searchd --prepare-root ROOT [CURSOR]\n",
                "Build missing preparation from retained bytes; search never rebuilds it.\n",
                "prepare-root handles a bounded batch; pass next_cursor until exhausted=true.\n",
                "Changed catalog/profile invalidates the cursor; restart without it. Gaps stay explicit.\n",
                "\nOFFLINE SOURCE-MAPPING PLAN:\n",
                "  eliot-searchd --plan-control-migration ROOT TARGET_NAMESPACE_UUID OUTPUT_DIRECTORY\n",
                "Requires an existing unowned root and an existing, separate output directory.\n",
                "Preserves source state; writes a mapping draft only, without redb import or cutover.\n"
            )
        );
    }
    result
}

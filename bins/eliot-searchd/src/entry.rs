//! Primary ELIOT Search daemon entrypoint.
//!
//! The primary binary composes the owner-fenced persistent DIRECT runtime,
//! bounded continuation windows, opaque source handles, directory manifests,
//! guarded maintenance and platform revision protection. Canonical provider IPC
//! is installation-scoped local transport. The obsolete TCP/token-file proxy is
//! compiled only with `legacy-loopback-harness` for process fixtures.

#![deny(unsafe_code)]

#[cfg(feature = "wave4-query")]
#[allow(dead_code)]
mod access_composition;
mod app;
#[cfg(feature = "legacy-loopback-harness")]
mod authenticated_proxy;
mod catalog_presence;
mod catalog_quarantine;
mod config_composition;
mod continuation;
mod development;
mod direct_preparation;
#[path = "secure_direct_store.rs"]
mod direct_store;
mod directory_manifest;
#[cfg(feature = "legacy-loopback-harness")]
mod endpoint;
mod maintenance;
mod maintenance_guard;
pub(crate) mod owner_composition;
#[path = "direct_store.rs"]
mod plaintext_direct_store;
mod preparation_composition;
#[cfg(feature = "wave3-index")]
#[allow(dead_code)]
mod projection_composition;
mod protocol_io;
mod provider_composition;
mod qualified_entropy;
#[cfg(feature = "wave4-query")]
#[allow(dead_code)]
mod query_composition;
#[cfg(feature = "wave4-query")]
#[path = "query_composition/live_authority.rs"]
mod query_live_authority;
#[cfg(feature = "wave4-query")]
#[path = "query_composition/serving.rs"]
mod query_serving_composition;
mod public_runtime_service;
#[allow(dead_code)]
mod publication_composition;
#[cfg(feature = "wave3-index")]
#[allow(dead_code)]
mod rebuild_composition;
#[cfg(feature = "wave7-lifecycle")]
#[allow(dead_code)]
mod restore_composition;
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
#[allow(dead_code)]
#[path = "source_composition.rs"]
mod git_source_composition;
#[path = "direct_store/composition.rs"]
mod source_composition;
mod source_fence;
mod source_migration_command;
mod source_roots;
mod storage_security;

#[cfg(all(test, windows))]
mod protected_ingest_tests;
#[cfg(all(test, windows))]
mod root_draining_266;

use std::process::ExitCode;

#[cfg(feature = "legacy-loopback-harness")]
fn maybe_run_legacy_loopback() -> Option<ExitCode> {
    authenticated_proxy::maybe_run()
}

#[cfg(not(feature = "legacy-loopback-harness"))]
const fn maybe_run_legacy_loopback() -> Option<ExitCode> {
    None
}

fn main() -> ExitCode {
    let result = source_migration_command::maybe_run()
        .or_else(preparation_composition::maybe_run)
        .or_else(maybe_run_legacy_loopback)
        .or_else(public_runtime_service::maybe_run)
        .or_else(secure_commands::maybe_run)
        .unwrap_or_else(app::run_main);
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
                "Preserves source state; writes a mapping draft only, without redb import or cutover.\n",
                "\nPROVIDER TRANSPORT:\n",
                "  Canonical startup uses installation-scoped local IPC.\n",
                "  TCP ports, endpoint files and token files are not accepted by the primary build.\n"
            )
        );
    }
    result
}

//! Primary ELIOT Search CLI entrypoint.
//!
//! The primary client exposes the persistent DIRECT command surface, the
//! interactive paged runtime with opaque source handles, and the canonical local
//! provider bootstrap. Obsolete address/token-file routing is available only in
//! the explicit `legacy-loopback-harness` feature for process fixtures.

#![forbid(unsafe_code)]

mod app;
mod endpoint_client;
#[cfg(feature = "legacy-loopback-harness")]
mod native_bootstrap;
mod native_credentials;
mod native_local_registered;
mod native_registered;
mod provider_client;
mod public_client;
#[cfg(feature = "legacy-loopback-harness")]
mod remote_client;

use std::process::ExitCode;

#[cfg(feature = "legacy-loopback-harness")]
fn maybe_run_legacy_remote() -> Option<ExitCode> {
    remote_client::maybe_run()
}

#[cfg(not(feature = "legacy-loopback-harness"))]
const fn maybe_run_legacy_remote() -> Option<ExitCode> {
    None
}

fn main() -> ExitCode {
    maybe_run_legacy_remote()
        .or_else(public_client::maybe_run)
        .unwrap_or_else(app::run_main)
}

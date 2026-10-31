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

fn main() -> ExitCode {
    app::run_main()
}

//! Primary ELIOT Search CLI entrypoint.
//!
//! The primary client exposes the persistent DIRECT command surface, the
//! interactive paged runtime with opaque source handles, and authenticated
//! loopback one-shot commands. The earlier immutable snapshot/BM25 client
//! remains `eliot-search-snapshot`.

#![forbid(unsafe_code)]

mod app;
mod endpoint_client;
mod public_client;
mod remote_client;

use std::process::ExitCode;

fn main() -> ExitCode {
    remote_client::maybe_run()
        .or_else(public_client::maybe_run)
        .unwrap_or_else(app::run_main)
}

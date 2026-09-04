//! Primary ELIOT Search CLI entrypoint.
//!
//! The primary client exposes the persistent DIRECT command surface and the
//! interactive paged runtime with opaque source handles. The earlier immutable
//! snapshot/BM25 client remains available as `eliot-search-snapshot`.

#![forbid(unsafe_code)]

mod app;
mod public_client;

use std::process::ExitCode;

fn main() -> ExitCode {
    public_client::maybe_run().unwrap_or_else(app::run_main)
}

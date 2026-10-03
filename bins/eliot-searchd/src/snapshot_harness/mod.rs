//! Legacy loopback snapshot process composition.
//!
//! CLI parsing, owner/token state, framing, response projection and server
//! orchestration remain private responsibility modules for the regression
//! harness. No production entrypoint imports this tree.

mod config;
mod protocol;
mod response;
mod runtime;
mod state;

use std::process;

pub(crate) fn main() {
    match config::parse_options().and_then(runtime::run) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("eliot-searchd: {error}");
            process::exit(1);
        }
    }
}

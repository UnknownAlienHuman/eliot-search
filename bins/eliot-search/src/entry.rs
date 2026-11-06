//! Standalone root operator CLI entrypoint.

#![forbid(unsafe_code)]

mod app;

fn main() {
    std::process::exit(app::run_main());
}

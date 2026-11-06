//! Standalone root operator CLI entrypoint.

#![forbid(unsafe_code)]

mod app;

fn main() {
    // Timed-out stdio readers may still be blocked on inherited pipe handles.
    // This one-shot entry always terminates their process immediately.
    std::process::exit(app::run_main());
}

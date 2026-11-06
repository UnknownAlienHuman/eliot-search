//! Standalone root operator CLI entrypoint.

#![forbid(unsafe_code)]

mod app;
mod prepared_response;

fn main() {
    // Timed-out stdio readers may still be blocked on inherited pipe handles.
    // This one-shot entry always terminates their process immediately.
    std::process::exit(app::run_main());
}

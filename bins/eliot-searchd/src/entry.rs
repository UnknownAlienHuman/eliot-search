//! Standalone root operator binary entrypoint.

#![forbid(unsafe_code)]

mod app;
mod collect_objects;
mod decoded_exact;
mod exact_literal;
mod framed_search;
mod native_root_command;
mod prepared_source;
mod published_corpus;
mod raw_exact;
mod verified_query;

fn main() -> std::process::ExitCode {
    app::run_main()
}

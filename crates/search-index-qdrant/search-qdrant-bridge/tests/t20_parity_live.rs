//! T24 mandatory live T20 parity: denied documents cannot influence permitted
//! ranking or the scoped IDF denominator.
//!
//! Run: `cargo test -p search-qdrant-bridge --test t20_parity_live`.

#[path = "t20_parity_live/support.rs"]
mod support;
#[path = "t20_parity_live/scenario.rs"]
mod scenario;

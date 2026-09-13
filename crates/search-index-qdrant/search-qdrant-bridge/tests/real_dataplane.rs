//! T24 real data-plane adapter: live discriminating tests.
//!
//! Every live scenario spawns the exact qualified native server on disposable
//! storage, admits it through the executed T22 suite, then exercises the real
//! `qdrant-client` transport. The in-memory bridge remains a parity oracle only.
//!
//! Run: `cargo test -p search-qdrant-bridge --test real_dataplane`.

#[path = "real_dataplane/support.rs"]
mod support;
#[path = "real_dataplane/collection_name.rs"]
mod collection_name;
#[path = "real_dataplane/parity.rs"]
mod parity;
#[path = "real_dataplane/rejection.rs"]
mod rejection;
#[path = "real_dataplane/recovery.rs"]
mod recovery;
#[path = "real_dataplane/pagination.rs"]
mod pagination;
#[path = "real_dataplane/connection.rs"]
mod connection;

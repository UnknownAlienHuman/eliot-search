//! Bridge-owned capability probes over Qdrant's authenticated gRPC API.
//!
//! Operational admission runs through
//! [`BlockingRealQueryPlane::connect_and_probe`](crate::real::BlockingRealQueryPlane::connect_and_probe)
//! against an already-supervised endpoint and does not manage a process. The
//! [`DisposableServer`] path is a fixture for running the same probe suite in
//! tests; process lifecycle remains outside the bridge's operational path.
//!
//! Passing these probes establishes operational endpoint behavior only. It is
//! not release qualification. Vendor types remain private to this package.

mod error;
mod fixtures;
mod probes;
mod server;
mod suite;

/// Fixed disposable-fixture credential. This is never used outside the
/// private native-server fixture and is passed to its child only via env.
pub(crate) const FIXTURE_API_KEY: &str = "eliot-qdrant-live-fixture-key-v1";

pub use error::LiveError;
pub use fixtures::{
    ACCESS_A, EPOCH_MAX, EPOCH_MIN, FIELD_ACCESS, FIELD_FROM, FIELD_TENANT, FIELD_UNINDEXED,
    FIELD_UNTIL, QUALIFICATION_COLLECTION, TENANT_A, TENANT_B, UUID_POINT, VECTOR_CODE,
    VECTOR_TEXT, VISIBLE_EPOCH, VISIBLE_EPOCH_I64,
};
pub use server::{
    DisposableServer, LiveEndpoint, NATIVE_EXE_PATH, free_loopback_ports, spawn_disposable_server,
    verify_executable,
};
pub(crate) use suite::{ExecutedProbeReceipt, run_live_probe_suite};
pub use suite::{LiveSuiteReport, run_qualification_suite, verify_compiled_client};

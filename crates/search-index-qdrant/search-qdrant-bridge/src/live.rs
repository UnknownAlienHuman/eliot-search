//! Live T22 qualification path beside the in-memory oracle.
//!
//! This module spawns the exact qualified native server on disposable
//! storage with OS-assigned loopback ports, executes every bridge-owned
//! mandatory probe through the pinned `qdrant-client` gRPC transport, then
//! kills the server and removes the storage. Indexed admission still requires
//! [`QualifiedGate`](crate::qualified::QualifiedGate), which rechecks the
//! executed [`LiveProbeReceipt`](crate::qualified::LiveProbeReceipt).
//!
//! Vendor types remain private to this package. Public callers consume only
//! Eliot-owned qualification types and the stable live-path wrappers re-exported
//! below.

mod error;
mod fixtures;
mod probes;
mod server;
mod suite;

pub use error::LiveError;
pub use fixtures::{
    ACCESS_A, EPOCH_MAX, EPOCH_MIN, FIELD_ACCESS, FIELD_FROM, FIELD_TENANT,
    FIELD_UNINDEXED, FIELD_UNTIL, QUALIFICATION_COLLECTION, TENANT_A, TENANT_B,
    UUID_POINT, VECTOR_CODE, VECTOR_TEXT, VISIBLE_EPOCH, VISIBLE_EPOCH_I64,
};
pub use server::{
    DisposableServer, LiveEndpoint, NATIVE_EXE_PATH, free_loopback_ports,
    spawn_disposable_server, verify_executable,
};
pub use suite::{
    LiveSuiteReport, run_qualification_suite, verify_compiled_client,
};

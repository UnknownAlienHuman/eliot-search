//! Disposable qualification-server boundary.
//!
//! This module is a qualification harness only. Product process ownership
//! remains in `search-qdrant-supervisor`; the bridge owns exact artifact
//! measurement, loopback endpoint construction and bounded disposable-probe
//! orchestration.

mod artifact;
mod diagnostics;
mod endpoint;
mod process;

pub use artifact::verify_executable;
pub use endpoint::{LiveEndpoint, free_loopback_ports};
pub use process::{DisposableServer, spawn_disposable_server};
pub(super) use endpoint::connect;

/// Pinned native server under qualification.
pub const NATIVE_EXE_PATH: &str = r"C:\Tools\Qdrant\1.19.0\qdrant.exe";

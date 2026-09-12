//! Exact T22 artifact/client/IDF qualification gate.
//!
//! This facade owns every pinned Qdrant server/client identity in one upgrade
//! surface. Bounded submodules own artifact verification, client verification,
//! independent-IDF admission, live-probe admission and stable errors. None of
//! them performs I/O, starts a process or falls back to the in-memory oracle.

mod artifact;
mod client;
mod error;
mod gate;
mod idf;

pub use artifact::{ArtifactReceipt, ObservedArtifact, verify_artifact};
pub use client::{ClientReceipt, ObservedClient, verify_client};
pub use error::QualificationError;
pub use gate::{
    LiveProbeOutcome, LiveProbeReceipt, MANDATORY_LIVE_PROBES, QualifiedGate,
};
pub use idf::{
    BaseEligibility, IndependentIdfProfile, IndependentIdfReceipt,
    admit_independent_idf,
};

/// Qualified Qdrant server version (`qdrant.exe --version` / `GET /`).
pub const QUALIFIED_SERVER_VERSION: &str = "1.19.0";
/// Qualified server build identity (upstream commit tag `74f3e85b`).
pub const QUALIFIED_SERVER_BUILD: &str = "74f3e85b";
/// Qualified executable SHA-256 (uppercase hex, `84_184_576` bytes).
pub const QUALIFIED_EXE_SHA256_HEX: &str =
    "369C562EAE3D89333A13ABFDB522FA209E3F587C1217A1059D817E80814EA9D4";
/// Qualified executable byte length.
pub const QUALIFIED_EXE_BYTES: u64 = 84_184_576;
/// Qualified target architecture.
pub const QUALIFIED_ARCH: &str = "x86_64";
/// Qualified target OS.
pub const QUALIFIED_OS: &str = "windows";
/// Qualified server license receipt class.
pub const QUALIFIED_SERVER_LICENSE: &str = "Apache-2.0";

/// Qualified Rust client crate.
pub const QUALIFIED_CLIENT_CRATE: &str = "qdrant-client";
/// Qualified Rust client version.
///
/// Exact `major.minor.patch` match with the server. Upstream compatibility
/// tolerates a wider range, but this qualification relies on the exact 1.19
/// sparse-IDF protobuf surface.
pub const QUALIFIED_CLIENT_VERSION: &str = "1.19.0";
/// crates.io source checksum (`.crate` SHA-256) for the qualified client.
pub const QUALIFIED_CLIENT_CHECKSUM: &str =
    "dddc19df129bad7346ebd027288621ab1ac7e52678371f906b9a8622d7aaf87e";
/// Upstream VCS identity of the qualified client sources.
pub const QUALIFIED_CLIENT_GIT_SHA: &str = "7c838035ae7b9455636dcaca918a55b7d7ca638f";
/// Qualified Rust client license.
pub const QUALIFIED_CLIENT_LICENSE: &str = "Apache-2.0";

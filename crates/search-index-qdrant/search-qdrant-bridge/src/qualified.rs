//! Selected Qdrant identity constants and focused validation helpers.
//!
//! These helpers support artifact diagnostics, client checks, IDF-profile
//! validation and live behavior tests. Their outputs are not runtime
//! credentials: the real data plane constructs from selected configuration and
//! current server observation.

mod artifact;
mod client;
mod error;
mod gate;
mod idf;

pub use artifact::{ArtifactReceipt, ObservedArtifact, verify_artifact};
pub use client::{ClientReceipt, ObservedClient, verify_client};
pub use error::QualificationError;
pub use gate::{
    LiveProbeOutcome, LiveProbeReceipt, MANDATORY_LIVE_PROBES,
    validate_live_probe_receipt,
};
pub use idf::{
    BaseEligibility, IndependentIdfProfile, IndependentIdfReceipt,
    admit_independent_idf,
};

/// Selected Qdrant server version.
pub const QUALIFIED_SERVER_VERSION: &str = "1.19.0";
/// Selected server build identity (upstream commit tag `74f3e85b`).
pub const QUALIFIED_SERVER_BUILD: &str = "74f3e85b";
/// Selected executable SHA-256 (uppercase hex, `84_184_576` bytes).
pub const QUALIFIED_EXE_SHA256_HEX: &str =
    "369C562EAE3D89333A13ABFDB522FA209E3F587C1217A1059D817E80814EA9D4";
/// Selected executable byte length.
pub const QUALIFIED_EXE_BYTES: u64 = 84_184_576;
/// Selected target architecture for the optional managed binary.
pub const QUALIFIED_ARCH: &str = "x86_64";
/// Selected target OS for the optional managed binary.
pub const QUALIFIED_OS: &str = "windows";
/// Selected server license class.
pub const QUALIFIED_SERVER_LICENSE: &str = "Apache-2.0";

/// Selected Rust client crate.
pub const QUALIFIED_CLIENT_CRATE: &str = "qdrant-client";
/// Selected Rust client version.
pub const QUALIFIED_CLIENT_VERSION: &str = "1.19.0";
/// crates.io source checksum (`.crate` SHA-256) for the selected client.
pub const QUALIFIED_CLIENT_CHECKSUM: &str =
    "dddc19df129bad7346ebd027288621ab1ac7e52678371f906b9a8622d7aaf87e";
/// Upstream VCS identity of the selected client sources.
pub const QUALIFIED_CLIENT_GIT_SHA: &str = "7c838035ae7b9455636dcaca918a55b7d7ca638f";
/// Selected Rust client license.
pub const QUALIFIED_CLIENT_LICENSE: &str = "Apache-2.0";

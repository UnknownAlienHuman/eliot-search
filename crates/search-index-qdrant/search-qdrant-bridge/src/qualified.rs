//! Exact T22 artifact/client/IDF qualification gate.
//!
//! This module owns the pinned identity of the one qualified Qdrant
//! server/client/IDF profile and the admission checks in front of every live
//! call. It performs no I/O, spawns no process and never falls back: any
//! drift in version, build, digest, size, architecture, OS, client revision
//! or IDF shape fails with a typed error.
//!
//! The in-memory oracle in [`crate`] remains the behavioral reference; the
//! live path in [`crate::live`] is usable only through [`QualifiedGate`],
//! which requires an executed [`LiveProbeReceipt`] with all mandatory probes
//! passed against the exact artifact below.

use core::fmt;

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
/// Exact `major.minor.patch` match with the server. Upstream `is_compatible`
/// tolerates ±1 minor with a warning, which is insufficient here: the 1.19
/// proto fields this qualification relies on (`SearchParams.idf`,
/// `IdfParams.corpus`) do not exist on older clients, and a newer client may
/// speak fields the qualified server does not know.
pub const QUALIFIED_CLIENT_VERSION: &str = "1.19.0";
/// crates.io source checksum (`.crate` SHA-256) for `qdrant-client` 1.19.0.
pub const QUALIFIED_CLIENT_CHECKSUM: &str =
    "dddc19df129bad7346ebd027288621ab1ac7e52678371f906b9a8622d7aaf87e";
/// Upstream VCS identity of the qualified client sources.
pub const QUALIFIED_CLIENT_GIT_SHA: &str = "7c838035ae7b9455636dcaca918a55b7d7ca638f";
/// Qualified Rust client license.
pub const QUALIFIED_CLIENT_LICENSE: &str = "Apache-2.0";

/// Closed qualification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationError {
    ServerVersionMismatch,
    ServerBuildMismatch,
    ArtifactDigestMismatch,
    ArtifactSizeMismatch,
    ArchitectureMismatch,
    OsMismatch,
    ClientCrateMismatch,
    ClientVersionMismatch,
    ClientChecksumMismatch,
    /// A local IDF factor or local corpus statistics were supplied next to
    /// the Qdrant-side `idf` modifier: IDF would be applied twice.
    DoubleIdf,
    /// The sparse vector is not configured with the Qdrant `idf` modifier.
    IdfModifierMissing,
    /// The `idf.corpus` population filter does not share the exact canonical
    /// base eligibility plan with retrieval.
    CorpusEligibilityDiverged,
    InvalidVectorName,
    /// A mandatory live probe is missing or did not pass.
    MandatoryProbeFailed,
    /// The live backend reports a different server identity than qualified.
    LiveIdentityMismatch,
}

impl QualificationError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ServerVersionMismatch => "QDRANT_QUAL_SERVER_VERSION_MISMATCH",
            Self::ServerBuildMismatch => "QDRANT_QUAL_SERVER_BUILD_MISMATCH",
            Self::ArtifactDigestMismatch => "QDRANT_QUAL_ARTIFACT_DIGEST_MISMATCH",
            Self::ArtifactSizeMismatch => "QDRANT_QUAL_ARTIFACT_SIZE_MISMATCH",
            Self::ArchitectureMismatch => "QDRANT_QUAL_ARCHITECTURE_MISMATCH",
            Self::OsMismatch => "QDRANT_QUAL_OS_MISMATCH",
            Self::ClientCrateMismatch => "QDRANT_QUAL_CLIENT_CRATE_MISMATCH",
            Self::ClientVersionMismatch => "QDRANT_QUAL_CLIENT_VERSION_MISMATCH",
            Self::ClientChecksumMismatch => "QDRANT_QUAL_CLIENT_CHECKSUM_MISMATCH",
            Self::DoubleIdf => "QDRANT_QUAL_DOUBLE_IDF",
            Self::IdfModifierMissing => "QDRANT_QUAL_IDF_MODIFIER_MISSING",
            Self::CorpusEligibilityDiverged => "QDRANT_QUAL_CORPUS_ELIGIBILITY_DIVERGED",
            Self::InvalidVectorName => "QDRANT_QUAL_INVALID_VECTOR_NAME",
            Self::MandatoryProbeFailed => "QDRANT_QUAL_MANDATORY_PROBE_FAILED",
            Self::LiveIdentityMismatch => "QDRANT_QUAL_LIVE_IDENTITY_MISMATCH",
        }
    }
}

impl fmt::Display for QualificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for QualificationError {}

/// Observed server artifact identity (measured, never assumed).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedArtifact {
    pub version: String,
    pub build: String,
    pub exe_sha256_hex: String,
    pub exe_bytes: u64,
    pub arch: String,
    pub os: String,
}

/// Accepted server artifact receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactReceipt {
    pub server_version: String,
    pub server_build: String,
}

/// Verifies one observed server artifact against the exact qualified identity.
pub fn verify_artifact(observed: &ObservedArtifact) -> Result<ArtifactReceipt, QualificationError> {
    if observed.version != QUALIFIED_SERVER_VERSION {
        return Err(QualificationError::ServerVersionMismatch);
    }
    if observed.build != QUALIFIED_SERVER_BUILD {
        return Err(QualificationError::ServerBuildMismatch);
    }
    if observed.exe_sha256_hex != QUALIFIED_EXE_SHA256_HEX {
        return Err(QualificationError::ArtifactDigestMismatch);
    }
    if observed.exe_bytes != QUALIFIED_EXE_BYTES {
        return Err(QualificationError::ArtifactSizeMismatch);
    }
    if observed.arch != QUALIFIED_ARCH {
        return Err(QualificationError::ArchitectureMismatch);
    }
    if observed.os != QUALIFIED_OS {
        return Err(QualificationError::OsMismatch);
    }
    Ok(ArtifactReceipt {
        server_version: observed.version.clone(),
        server_build: observed.build.clone(),
    })
}

/// Observed Rust client identity (resolved from the registry/lockfile).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedClient {
    pub crate_name: String,
    pub version: String,
    pub source_checksum: String,
}

/// Accepted Rust client receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientReceipt {
    pub client_version: String,
}

/// Verifies one observed Rust client against the exact qualified pin.
pub fn verify_client(observed: &ObservedClient) -> Result<ClientReceipt, QualificationError> {
    if observed.crate_name != QUALIFIED_CLIENT_CRATE {
        return Err(QualificationError::ClientCrateMismatch);
    }
    if observed.version != QUALIFIED_CLIENT_VERSION {
        return Err(QualificationError::ClientVersionMismatch);
    }
    if observed.source_checksum != QUALIFIED_CLIENT_CHECKSUM {
        return Err(QualificationError::ClientChecksumMismatch);
    }
    Ok(ClientReceipt {
        client_version: observed.version.clone(),
    })
}

/// Canonical base eligibility plan shared by retrieval and the IDF corpus.
///
/// The bridge stays lexical-agnostic: eligibility is the closed triple the
/// qualification fixtures vary (partition, tenant, epoch). Retrieval and
/// `idf.corpus` are specified independently but must carry the identical
/// triple, so IDF denominators can never silently diverge from what the
/// caller is allowed to see.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseEligibility {
    pub access_partition: String,
    pub tenant: String,
    pub visible_epoch: u64,
}

/// Independent-IDF profile descriptor: TF-only vectors plus Qdrant-side IDF.
///
/// Frozen lexical profiles emit raw term weights with a local IDF factor of
/// exactly `1.0` and no local corpus statistics (`IdfMode::DelegatedToQdrant`
/// in `search-lexical`); the single IDF application happens inside Qdrant via
/// the sparse `idf` modifier over the `idf.corpus` population.
#[derive(Clone, Debug, PartialEq)]
pub struct IndependentIdfProfile {
    pub vector_name: String,
    /// Local IDF multiplier. Must be exactly `1.0` (delegated, no local IDF).
    pub local_idf_factor: f32,
    /// Whether local corpus statistics were supplied. Must be `false`.
    pub local_statistics_present: bool,
    /// Whether the Qdrant sparse vector carries `modifier: idf`. Must be `true`.
    pub qdrant_modifier_idf: bool,
    /// Retrieval eligibility filter plan.
    pub retrieval_eligibility: BaseEligibility,
    /// `idf.corpus` population filter plan. Must equal retrieval eligibility.
    pub idf_corpus_eligibility: BaseEligibility,
}

/// Accepted independent-IDF receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndependentIdfReceipt {
    pub vector_name: String,
    /// Always `true`: admission is proof that double IDF is impossible.
    pub double_idf_impossible: bool,
}

/// Admits one independent-IDF profile. Any local IDF contribution or any
/// divergence between the retrieval plan and the IDF corpus plan fails.
pub fn admit_independent_idf(
    profile: &IndependentIdfProfile,
) -> Result<IndependentIdfReceipt, QualificationError> {
    if profile.vector_name.is_empty() {
        return Err(QualificationError::InvalidVectorName);
    }
    // Exact comparison is intentional: the delegated local factor is defined
    // as precisely `1.0`, so any deviation — however small — is double IDF.
    #[allow(clippy::float_cmp)]
    if profile.local_idf_factor != 1.0 || profile.local_statistics_present {
        return Err(QualificationError::DoubleIdf);
    }
    if !profile.qdrant_modifier_idf {
        return Err(QualificationError::IdfModifierMissing);
    }
    if profile.retrieval_eligibility != profile.idf_corpus_eligibility {
        return Err(QualificationError::CorpusEligibilityDiverged);
    }
    Ok(IndependentIdfReceipt {
        vector_name: profile.vector_name.clone(),
        double_idf_impossible: true,
    })
}

/// Bridge-owned mandatory live probes for the T22 profile.
///
/// Supervisor/lexical/daemon-owned probes from `qualification/qdrant/probes.toml`
/// are out of scope here and are not admitted by this gate.
pub const MANDATORY_LIVE_PROBES: [&str; 13] = [
    "live_server_identity",
    "one_shard_topology",
    "payload_index_completeness",
    "strict_unindexed_retrieve_rejected",
    "strict_unindexed_update_rejected",
    "signed_i64_epoch_range",
    "missing_valid_until_open_end",
    "sparse_idf_modifier",
    "independent_idf_population_filter",
    "wait_true_mutation_ack",
    "strong_write_ordering",
    "exact_count_and_readback",
    "schema_digest_equality",
];

/// One executed live probe outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProbeOutcome {
    pub probe_id: String,
    pub passed: bool,
    pub detail: String,
}

/// Executed live-probe receipt.
///
/// Constructed only by running the live suite in [`crate::live`] against the
/// exact qualified artifact; the gate below rechecks versions and mandatory
/// outcomes so a tampered or partial receipt can never admit the live path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProbeReceipt {
    pub server_version: String,
    pub server_build: String,
    pub client_version: String,
    pub collection: String,
    pub outcomes: Vec<LiveProbeOutcome>,
}

/// Explicit admission token for the live Qdrant path.
///
/// There is no default, no fallback and no mock constructor: the only way to
/// obtain a gate is [`QualifiedGate::admit`], which requires the exact
/// qualified server/client identities plus every mandatory probe passed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedGate {
    server_version: String,
    collection: String,
}

impl QualifiedGate {
    /// Admits the live path after rechecking the executed receipt.
    pub fn admit(receipt: &LiveProbeReceipt) -> Result<Self, QualificationError> {
        if receipt.server_version != QUALIFIED_SERVER_VERSION
            || receipt.server_build != QUALIFIED_SERVER_BUILD
        {
            return Err(QualificationError::LiveIdentityMismatch);
        }
        if receipt.client_version != QUALIFIED_CLIENT_VERSION {
            return Err(QualificationError::LiveIdentityMismatch);
        }
        for required in MANDATORY_LIVE_PROBES {
            let passed = receipt
                .outcomes
                .iter()
                .any(|outcome| outcome.probe_id == required && outcome.passed);
            if !passed {
                return Err(QualificationError::MandatoryProbeFailed);
            }
        }
        Ok(Self {
            server_version: receipt.server_version.clone(),
            collection: receipt.collection.clone(),
        })
    }

    /// Collection the gate was executed against.
    #[must_use]
    pub fn collection(&self) -> &str {
        &self.collection
    }

    /// Admitted server version.
    #[must_use]
    pub fn server_version(&self) -> &str {
        &self.server_version
    }
}

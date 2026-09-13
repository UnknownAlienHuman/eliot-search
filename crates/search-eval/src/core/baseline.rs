//! Qualified A/B/C baseline and candidate descriptors.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::EvalError;
use super::run::FrozenRunManifest;

/// Role of one compared implementation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BaselineRole {
    /// Baseline A.
    A,
    /// Baseline B.
    B,
    /// Candidate C.
    C,
}

/// Exact qualified baseline/candidate descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaselineDescriptor {
    /// Stable baseline identity.
    pub baseline_id: OpaqueId,
    /// A/B/C role.
    pub role: BaselineRole,
    /// Exact source or release identity.
    pub source_identity: OpaqueId,
    /// Exact version identity; `latest` is forbidden.
    pub version_identity: OpaqueId,
    /// Exact artifact digest.
    pub artifact_digest: Blake3Digest32,
    /// Exact configuration digest.
    pub configuration_digest: Blake3Digest32,
    /// Exact invocation-driver digest.
    pub driver_digest: Blake3Digest32,
    /// Exact declared scope capability digest.
    pub scope_digest: Blake3Digest32,
    /// Frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Whether artifact qualification succeeded.
    pub qualified: bool,
    /// Whether network access is disabled.
    pub no_network: bool,
    /// Whether hidden patches are absent.
    pub unmodified_artifact: bool,
    /// Qualification evidence.
    pub qualification_receipt: ReceiptRef,
}

/// Baseline accepted for one exact frozen run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedBaseline(BaselineDescriptor);

impl ValidatedBaseline {
    /// Exact accepted descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &BaselineDescriptor {
        &self.0
    }
}

/// Validates exact artifact, version, scope, driver, and run binding.
pub fn validate_baseline_descriptor(
    descriptor: BaselineDescriptor,
    run: &FrozenRunManifest,
) -> Result<ValidatedBaseline, EvalError> {
    let version = descriptor.version_identity.as_str();
    if !descriptor.qualified
        || !descriptor.no_network
        || !descriptor.unmodified_artifact
        || descriptor.run_digest != run.run_digest()
        || descriptor.source_identity.as_str().is_empty()
        || version.is_empty()
        || version.eq_ignore_ascii_case("latest")
        || descriptor.qualification_receipt.as_str().is_empty()
    {
        return Err(EvalError::BaselineUnqualified);
    }
    Ok(ValidatedBaseline(descriptor))
}

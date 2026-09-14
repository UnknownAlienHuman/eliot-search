//! Domain-separated content-addressed revision object identity.

use search_contracts::Blake3Digest32;

use super::error::RevisionStoreError;
use super::limits::CAS_ADDRESS_VERSION;
use super::residency::ResidencyClosure;

/// Closed revision object kind carried by a [`CasObjectAddress`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RevisionObjectKind {
    /// Version-one authenticated-encryption revision envelope object.
    RevisionEnvelopeV1,
}

impl RevisionObjectKind {
    /// Stable path segment for this object kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RevisionEnvelopeV1 => "revision-envelope-v1",
        }
    }
}

/// Domain-separated content address for one immutable revision object.
///
/// The address binds the complete [`ResidencyClosure`], the object kind, the
/// content digest, and the schema version. It carries no source path or
/// display name and cannot collide across inequivalent residency closures.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CasObjectAddress {
    /// Residency closure authorizing storage of this object.
    pub residency: ResidencyClosure,
    /// Closed object kind.
    pub kind: RevisionObjectKind,
    /// Exact content digest of the addressed bytes.
    pub content_digest: Blake3Digest32,
    /// Address schema version; must equal [`CAS_ADDRESS_VERSION`].
    pub schema_version: u16,
}

impl CasObjectAddress {
    /// Renders the deterministic slash-separated object path for adapters.
    ///
    /// Every domain appears as its own hyphenated-UUID segment, so filesystem
    /// or object-store layouts derived from this path inherit residency
    /// separation without further checks.
    pub fn to_path_string(&self) -> String {
        format!(
            "cas/v{}/{}/scope-{}/access-{}/confidentiality-{}/encryption-key-{}/retention-{}/erasure-{}/blake3-{}",
            self.schema_version,
            self.kind.as_str(),
            self.residency.scope,
            self.residency.access,
            self.residency.confidentiality,
            self.residency.encryption_key,
            self.residency.retention,
            self.residency.erasure,
            self.content_digest,
        )
    }
}

/// Derives the domain-separated address for one revision object.
///
/// Fails closed on an unsupported schema version or on the all-zero digest:
/// neither may address a real object.
pub fn derive_object_address(
    residency: &ResidencyClosure,
    kind: RevisionObjectKind,
    content_digest: Blake3Digest32,
    schema_version: u16,
) -> Result<CasObjectAddress, RevisionStoreError> {
    if schema_version != CAS_ADDRESS_VERSION {
        return Err(RevisionStoreError::AddressInvalid);
    }
    if content_digest.as_bytes() == &[0; 32] {
        return Err(RevisionStoreError::AddressInvalid);
    }
    Ok(CasObjectAddress {
        residency: *residency,
        kind,
        content_digest,
        schema_version,
    })
}

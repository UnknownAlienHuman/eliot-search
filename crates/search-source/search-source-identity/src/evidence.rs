//! Explicit filesystem, path, and stable-identity observations.

use core::fmt;

use search_contracts::{
    Blake3Digest32, CatalogRevision, NonZeroRevision, RepositoryLineageId, RootBindingId,
    SourceId, SourceIdentityKind,
};

use crate::IdentityError;

/// Maximum UTF-8 bytes in one qualified root-relative lookup path.
pub const MAX_IDENTITY_PATH_BYTES: usize = 32_768;

/// Case behavior proven by a qualified filesystem adapter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CaseBehavior {
    /// Lookup preserves case and treats distinct spellings as distinct keys.
    Sensitive,
    /// ASCII case folding is sufficient for this qualified profile.
    InsensitiveAscii,
    /// A qualified adapter already supplied the filesystem-native lookup form.
    PreNormalizedInsensitive,
    /// Exact case behavior is unsupported or unknown.
    Unsupported,
}

/// Unicode lookup behavior proven by a qualified adapter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UnicodeBehavior {
    /// Scalar values are preserved exactly.
    PreserveScalarValues,
    /// Input is attested as NFC-normalized by a qualified adapter.
    PreNormalizedNfc,
    /// Input is attested as NFD-normalized by a qualified adapter.
    PreNormalizedNfd,
    /// Exact Unicode behavior is unsupported or unknown.
    Unsupported,
}

/// Availability policy for a load-bearing stable identity field.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StableFieldPolicy {
    /// The field must be present.
    Required,
    /// Explicit absence is accepted but prevents exact filesystem identity.
    OptionalExplicitUnavailable,
    /// Adapter support is absent or unqualified.
    Unsupported,
}

/// Hard-link identity behavior.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LinkBehavior {
    /// Exact volume/file identity proves hard-link equivalence.
    StablePhysicalIdentity,
    /// No stable hard-link proof is available; grouping is denied.
    NoStableLinkIdentity,
    /// Link behavior is unsupported or unknown.
    Unsupported,
}

/// Reparse/link traversal behavior at the identity boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReparseBehavior {
    /// Observations bind the already-opened final target.
    FinalTargetIdentity,
    /// Reparse traversal is rejected by the qualified adapter.
    Rejected,
    /// Exact reparse behavior is unsupported or unknown.
    Unsupported,
}

/// Exact filesystem behavior profile consumed by pure identity decisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesystemIdentityProfile {
    revision: NonZeroRevision,
    schema_digest: Blake3Digest32,
    case_behavior: CaseBehavior,
    unicode_behavior: UnicodeBehavior,
    volume_identity: StableFieldPolicy,
    file_identity: StableFieldPolicy,
    link_behavior: LinkBehavior,
    reparse_behavior: ReparseBehavior,
}

impl FilesystemIdentityProfile {
    /// Creates a fully explicit qualified profile.
    ///
    /// # Errors
    ///
    /// Any unsupported behavior rejects the profile; callers may not fall back
    /// to implicit host defaults.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        revision: NonZeroRevision,
        schema_digest: Blake3Digest32,
        case_behavior: CaseBehavior,
        unicode_behavior: UnicodeBehavior,
        volume_identity: StableFieldPolicy,
        file_identity: StableFieldPolicy,
        link_behavior: LinkBehavior,
        reparse_behavior: ReparseBehavior,
    ) -> Result<Self, IdentityError> {
        if case_behavior == CaseBehavior::Unsupported
            || unicode_behavior == UnicodeBehavior::Unsupported
            || volume_identity == StableFieldPolicy::Unsupported
            || file_identity == StableFieldPolicy::Unsupported
            || link_behavior == LinkBehavior::Unsupported
            || reparse_behavior == ReparseBehavior::Unsupported
        {
            return Err(IdentityError::FilesystemProfileUnsupported);
        }
        Ok(Self {
            revision,
            schema_digest,
            case_behavior,
            unicode_behavior,
            volume_identity,
            file_identity,
            link_behavior,
            reparse_behavior,
        })
    }

    /// Monotone profile revision.
    #[must_use]
    pub const fn revision(self) -> NonZeroRevision {
        self.revision
    }

    /// Digest of the exact profile schema and adapter qualification.
    #[must_use]
    pub const fn schema_digest(self) -> Blake3Digest32 {
        self.schema_digest
    }

    /// Qualified case behavior.
    #[must_use]
    pub const fn case_behavior(self) -> CaseBehavior {
        self.case_behavior
    }

    /// Qualified Unicode behavior.
    #[must_use]
    pub const fn unicode_behavior(self) -> UnicodeBehavior {
        self.unicode_behavior
    }

    /// Volume identity policy.
    #[must_use]
    pub const fn volume_identity(self) -> StableFieldPolicy {
        self.volume_identity
    }

    /// File identity policy.
    #[must_use]
    pub const fn file_identity(self) -> StableFieldPolicy {
        self.file_identity
    }

    /// Hard-link behavior.
    #[must_use]
    pub const fn link_behavior(self) -> LinkBehavior {
        self.link_behavior
    }

    /// Reparse behavior.
    #[must_use]
    pub const fn reparse_behavior(self) -> ReparseBehavior {
        self.reparse_behavior
    }

    fn requires_adapter_normalization(self) -> bool {
        self.case_behavior == CaseBehavior::PreNormalizedInsensitive
            || matches!(
                self.unicode_behavior,
                UnicodeBehavior::PreNormalizedNfc | UnicodeBehavior::PreNormalizedNfd
            )
    }
}

/// Already-captured path observation from a qualified final-handle adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathObservation {
    /// Admitted root binding.
    pub root_binding_id: RootBindingId,
    /// Qualified root-relative lookup path using `/` separators.
    pub root_relative_lookup_path: String,
    /// Exact filesystem profile revision used by the adapter.
    pub profile_revision: NonZeroRevision,
    /// Exact profile schema digest used by the adapter.
    pub profile_schema_digest: Blake3Digest32,
    /// Whether adapter-specific case/Unicode normalization was verified.
    pub normalization_attested: bool,
}

/// Versioned canonical path lookup key.
///
/// Text equality is lookup evidence only and never source identity proof.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalPathKey {
    root_binding_id: RootBindingId,
    profile_revision: NonZeroRevision,
    profile_schema_digest: Blake3Digest32,
    lookup_path: String,
}

impl CanonicalPathKey {
    /// Admitted root binding.
    #[must_use]
    pub const fn root_binding_id(&self) -> RootBindingId {
        self.root_binding_id
    }

    /// Exact profile revision used to derive this key.
    #[must_use]
    pub const fn profile_revision(&self) -> NonZeroRevision {
        self.profile_revision
    }

    /// Exact profile digest used to derive this key.
    #[must_use]
    pub const fn profile_schema_digest(&self) -> Blake3Digest32 {
        self.profile_schema_digest
    }

    /// Qualified root-relative lookup spelling.
    ///
    /// Callers must apply disclosure policy before returning this value outside
    /// the identity/control boundary.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.lookup_path
    }

    /// UTF-8 byte length of the lookup spelling.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lookup_path.len()
    }

    /// Returns whether the lookup spelling is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lookup_path.is_empty()
    }
}

impl fmt::Debug for CanonicalPathKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalPathKey")
            .field("root_binding_id", &self.root_binding_id)
            .field("profile_revision", &self.profile_revision)
            .field("profile_schema_digest", &self.profile_schema_digest)
            .field("lookup_path", &format_args!("<redacted:{} bytes>", self.lookup_path.len()))
            .finish()
    }
}

/// Derives a canonical lookup key from an explicit qualified observation.
///
/// # Errors
///
/// Rejects profile mismatch, missing normalization attestation, absolute or
/// escaping paths, backslash/non-canonical separators, NUL, empty segments,
/// and paths beyond the finite byte ceiling.
pub fn derive_canonical_path_key(
    observation: &PathObservation,
    profile: FilesystemIdentityProfile,
) -> Result<CanonicalPathKey, IdentityError> {
    if observation.profile_revision != profile.revision()
        || observation.profile_schema_digest != profile.schema_digest()
    {
        return Err(IdentityError::IdentityObservationInvalid);
    }
    if profile.requires_adapter_normalization() && !observation.normalization_attested {
        return Err(IdentityError::IdentityObservationInvalid);
    }
    let path = observation.root_relative_lookup_path.as_str();
    validate_relative_lookup_path(path)?;
    let lookup_path = match profile.case_behavior() {
        CaseBehavior::InsensitiveAscii => path.to_ascii_lowercase(),
        CaseBehavior::Sensitive | CaseBehavior::PreNormalizedInsensitive => path.to_owned(),
        CaseBehavior::Unsupported => {
            return Err(IdentityError::FilesystemProfileUnsupported);
        }
    };
    Ok(CanonicalPathKey {
        root_binding_id: observation.root_binding_id,
        profile_revision: profile.revision(),
        profile_schema_digest: profile.schema_digest(),
        lookup_path,
    })
}

fn validate_relative_lookup_path(path: &str) -> Result<(), IdentityError> {
    if path.is_empty()
        || path.len() > MAX_IDENTITY_PATH_BYTES
        || path.starts_with('/')
        || path.starts_with("//")
        || path.contains('\\')
        || path.as_bytes().contains(&0)
    {
        return Err(IdentityError::PathEscapesAdmittedRoot);
    }
    let mut segments = path.split('/');
    let Some(first) = segments.next() else {
        return Err(IdentityError::PathEscapesAdmittedRoot);
    };
    if first.is_empty()
        || first == "."
        || first == ".."
        || first.as_bytes().get(1) == Some(&b':')
    {
        return Err(IdentityError::PathEscapesAdmittedRoot);
    }
    if segments.any(|segment| segment.is_empty() || segment == "." || segment == "..") {
        return Err(IdentityError::PathEscapesAdmittedRoot);
    }
    Ok(())
}

/// Load-bearing stable identity key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StableIdentityKey {
    /// Exact local volume and file identity.
    Filesystem {
        /// Stable local volume identity digest.
        volume_identity: Blake3Digest32,
        /// Stable final-handle file identity digest.
        file_identity: Blake3Digest32,
        /// Optional reuse-resistant generation marker.
        generation: Option<u64>,
    },
    /// Exact Git object identity within repository lineage.
    GitObject {
        /// Repository lineage boundary.
        lineage_id: RepositoryLineageId,
        /// Exact object identity digest.
        object_identity: Blake3Digest32,
    },
    /// Exact imported object identity.
    Imported {
        /// Import-object identity digest.
        import_identity: Blake3Digest32,
    },
    /// Exact authenticated virtual snapshot identity.
    VirtualSnapshot {
        /// Attestation-bound snapshot identity digest.
        attestation_digest: Blake3Digest32,
    },
}

impl StableIdentityKey {
    /// Shared source identity kind.
    #[must_use]
    pub const fn identity_kind(self) -> SourceIdentityKind {
        match self {
            Self::Filesystem { .. } => SourceIdentityKind::NtfsFile,
            Self::GitObject { .. } => SourceIdentityKind::GitBlobLineage,
            Self::Imported { .. } => SourceIdentityKind::ImportedObject,
            Self::VirtualSnapshot { .. } => SourceIdentityKind::AdmittedVirtualSnapshot,
        }
    }
}

/// Missing load-bearing stable evidence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MissingIdentityEvidence {
    /// Stable local volume identity is unavailable.
    VolumeIdentity,
    /// Stable final-handle file identity is unavailable.
    FileIdentity,
    /// Repository lineage or exact Git object identity is unavailable.
    RepositoryObjectIdentity,
    /// Imported object identity is unavailable.
    ImportedObjectIdentity,
    /// Authenticated virtual-snapshot attestation is unavailable.
    VirtualSnapshotAttestation,
    /// Adapter profile does not support this source kind.
    UnsupportedSourceKind,
}

/// Stable identity evidence or explicit unavailability.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StableIdentityEvidence {
    /// Exact stable key is available.
    Exact(StableIdentityKey),
    /// Stable evidence is explicitly unavailable.
    Unavailable(MissingIdentityEvidence),
}

/// Confidence class for the complete observation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ObservationConfidence {
    /// All load-bearing components were read from exact final-handle/object evidence.
    Exact,
    /// Exact stable identity exists and independent evidence corroborates it.
    Corroborated,
    /// Some non-load-bearing hints are available but stable identity is absent.
    Partial,
    /// The adapter could not qualify the observation.
    Unsupported,
}

/// Already-captured source identity observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityObservation {
    /// Versioned path lookup key.
    pub path_key: CanonicalPathKey,
    /// Stable physical/logical evidence or explicit unavailability.
    pub stable_evidence: StableIdentityEvidence,
    /// Current content digest hint; never source identity by itself.
    pub content_digest_hint: Option<Blake3Digest32>,
    /// Reuse-resistant metadata generation when available.
    pub metadata_generation: Option<u64>,
    /// Current byte length hint.
    pub byte_length_hint: Option<u64>,
    /// Caller-claimed durable identity, if any.
    pub claimed_source_id: Option<SourceId>,
    /// Catalog revision used to load prior candidates.
    pub candidate_catalog_revision: CatalogRevision,
    /// Exact profile revision used by the observation.
    pub profile_revision: NonZeroRevision,
    /// Exact profile schema digest.
    pub profile_schema_digest: Blake3Digest32,
    /// Observation confidence.
    pub confidence: ObservationConfidence,
    /// Content-free digest of exact observation evidence.
    pub evidence_digest: Blake3Digest32,
}

/// Validated observation accepted by resolution functions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedIdentityObservation(IdentityObservation);

impl ValidatedIdentityObservation {
    /// Original validated observation.
    #[must_use]
    pub const fn as_inner(&self) -> &IdentityObservation {
        &self.0
    }

    /// Consumes the wrapper.
    #[must_use]
    pub fn into_inner(self) -> IdentityObservation {
        self.0
    }
}

/// Validates a complete observation against an exact filesystem profile.
///
/// # Errors
///
/// Profile mismatch, unsupported confidence, zero generation, and missing
/// required filesystem stable fields are rejected.
pub fn validate_identity_observation(
    observation: IdentityObservation,
    profile: FilesystemIdentityProfile,
) -> Result<ValidatedIdentityObservation, IdentityError> {
    if observation.profile_revision != profile.revision()
        || observation.profile_schema_digest != profile.schema_digest()
        || observation.path_key.profile_revision() != profile.revision()
        || observation.path_key.profile_schema_digest() != profile.schema_digest()
        || observation.confidence == ObservationConfidence::Unsupported
        || observation.metadata_generation == Some(0)
    {
        return Err(IdentityError::IdentityObservationInvalid);
    }
    if let StableIdentityEvidence::Exact(StableIdentityKey::Filesystem {
        volume_identity: _,
        file_identity: _,
        generation,
    }) = observation.stable_evidence
        && generation == Some(0)
    {
        return Err(IdentityError::IdentityObservationInvalid);
    }
    if matches!(
        observation.stable_evidence,
        StableIdentityEvidence::Unavailable(MissingIdentityEvidence::VolumeIdentity)
    ) && profile.volume_identity() == StableFieldPolicy::Required
    {
        return Err(IdentityError::SourceIdentityInsufficientEvidence);
    }
    if matches!(
        observation.stable_evidence,
        StableIdentityEvidence::Unavailable(MissingIdentityEvidence::FileIdentity)
    ) && profile.file_identity() == StableFieldPolicy::Required
    {
        return Err(IdentityError::SourceIdentityInsufficientEvidence);
    }
    Ok(ValidatedIdentityObservation(observation))
}

#[cfg(test)]
mod tests {
    use search_contracts::{Blake3Digest32, NonZeroRevision, RootBindingId};

    use super::{
        CaseBehavior, FilesystemIdentityProfile, LinkBehavior, PathObservation,
        ReparseBehavior, StableFieldPolicy, UnicodeBehavior, derive_canonical_path_key,
    };
    use crate::IdentityError;

    fn profile(case_behavior: CaseBehavior) -> FilesystemIdentityProfile {
        FilesystemIdentityProfile::new(
            NonZeroRevision::new(1).expect("revision"),
            Blake3Digest32::from_bytes([1; 32]),
            case_behavior,
            UnicodeBehavior::PreserveScalarValues,
            StableFieldPolicy::Required,
            StableFieldPolicy::Required,
            LinkBehavior::StablePhysicalIdentity,
            ReparseBehavior::FinalTargetIdentity,
        )
        .expect("profile")
    }

    #[test]
    fn relative_path_cannot_escape_root() {
        let observation = PathObservation {
            root_binding_id: RootBindingId::from_bytes([1; 16]),
            root_relative_lookup_path: "src/../secret".into(),
            profile_revision: NonZeroRevision::new(1).expect("revision"),
            profile_schema_digest: Blake3Digest32::from_bytes([1; 32]),
            normalization_attested: true,
        };
        assert_eq!(
            derive_canonical_path_key(&observation, profile(CaseBehavior::Sensitive)),
            Err(IdentityError::PathEscapesAdmittedRoot)
        );
    }

    #[test]
    fn case_profile_changes_lookup_key_not_physical_identity() {
        let observation = PathObservation {
            root_binding_id: RootBindingId::from_bytes([1; 16]),
            root_relative_lookup_path: "Src/Lib.rs".into(),
            profile_revision: NonZeroRevision::new(1).expect("revision"),
            profile_schema_digest: Blake3Digest32::from_bytes([1; 32]),
            normalization_attested: true,
        };
        let sensitive = derive_canonical_path_key(
            &observation,
            profile(CaseBehavior::Sensitive),
        )
        .expect("key");
        let insensitive = derive_canonical_path_key(
            &observation,
            profile(CaseBehavior::InsensitiveAscii),
        )
        .expect("key");
        assert_ne!(sensitive, insensitive);
        assert_eq!(insensitive.as_str(), "src/lib.rs");
    }
}

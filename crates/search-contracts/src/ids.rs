use crate::bounds::{
    BoundedText, MAX_HANDLE_TOKEN_BYTES, MAX_NAME_BYTES, MAX_PROFILE_ID_BYTES,
    MIN_HANDLE_TOKEN_BYTES,
};
use crate::canonical::{OpaqueRef, base64url_decode, base64url_encode, hex_decode, hex_encode};
use crate::{ContractError, ContractErrorKind};
use core::{fmt, str::FromStr};

/// Canonical 16-byte UUID storage shared by strongly typed identifiers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UuidBytes([u8; 16]);

impl UuidBytes {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn parse(value: &str) -> Result<Self, ContractError> {
        let bytes = value.as_bytes();
        if bytes.len() != 36
            || bytes.get(8) != Some(&b'-')
            || bytes.get(13) != Some(&b'-')
            || bytes.get(18) != Some(&b'-')
            || bytes.get(23) != Some(&b'-')
        {
            return Err(ContractError::new(
                ContractErrorKind::InvalidCharacter,
                "uuid",
            ));
        }
        let mut compact = [0_u8; 32];
        let mut index = 0;
        for (position, byte) in bytes.iter().copied().enumerate() {
            if matches!(position, 8 | 13 | 18 | 23) {
                continue;
            }
            if !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte) {
                return Err(ContractError::new(
                    ContractErrorKind::InvalidCharacter,
                    "uuid",
                ));
            }
            compact[index] = byte;
            index = index.saturating_add(1);
        }
        let mut output = [0_u8; 16];
        let (pairs, remainder) = compact.as_chunks::<2>();
        debug_assert!(remainder.is_empty());
        for (output_index, pair) in pairs.iter().enumerate() {
            let high = lower_hex(pair[0]).ok_or_else(|| ContractError::malformed("uuid"))?;
            let low = lower_hex(pair[1]).ok_or_else(|| ContractError::malformed("uuid"))?;
            output[output_index] = (high << 4) | low;
        }
        Ok(Self(output))
    }
}

impl fmt::Display for UuidBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = hex_encode(&self.0);
        write!(
            formatter,
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }
}

impl FromStr for UuidBytes {
    type Err = ContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

fn lower_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

macro_rules! uuid_newtype {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(UuidBytes);

            impl $name {
                #[must_use]
                pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                    Self(UuidBytes::from_bytes(bytes))
                }

                #[must_use]
                pub const fn as_bytes(&self) -> &[u8; 16] {
                    self.0.as_bytes()
                }

                pub fn parse(value: &str) -> Result<Self, ContractError> {
                    UuidBytes::parse(value).map(Self)
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.0.fmt(formatter)
                }
            }

            impl FromStr for $name {
                type Err = ContractError;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    Self::parse(value)
                }
            }
        )+
    };
}

uuid_newtype!(
    InstallationId,
    InstallationIncarnationId,
    DataRootId,
    BindingId,
    WorkspaceId,
    WorkspaceViewRevisionId,
    RootBindingId,
    PathBindingId,
    RepositoryLineageId,
    CollectionGenerationId,
    CorpusId,
    ReferencePortfolioId,
    SourceNamespaceId,
    SourceId,
    SourceMembershipId,
    ProjectionMembershipId,
    SourceRevisionId,
    MaterializationId,
    RepresentationId,
    UnitId,
    AccessPartitionId,
    ScoringPartitionId,
    ScoringDocumentId,
    AccessPolicyBindingId,
    ResidencyPolicyBindingId,
    ScopeDomainId,
    AccessDomainId,
    ConfidentialityDomainId,
    EncryptionKeyDomainId,
    RetentionDomainId,
    ErasureDomainId,
    GrantId,
    RequestId,
    PlanId,
    CandidateId,
    CutoverId,
    BufferSnapshotId,
    ImportedSnapshotId,
    HandleId,
    ContinuationId,
    PublicationIntentId,
    PublicationReceiptId,
);

/// Non-zero runtime owner epoch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnerEpoch(u64);

impl OwnerEpoch {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if value == 0 {
            return Err(ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "owner_epoch",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, ContractError> {
        self.0
            .checked_add(1)
            .ok_or_else(|| ContractError::new(ContractErrorKind::EpochExhausted, "owner_epoch"))
            .and_then(Self::new)
    }
}

/// Non-negative epoch representable by signed 64-bit persistence layers.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Epoch(i64);

impl Epoch {
    pub fn new(value: i64) -> Result<Self, ContractError> {
        if !(0..i64::MAX).contains(&value) {
            return Err(ContractError::new(
                ContractErrorKind::EpochOutOfRange,
                "epoch",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, ContractError> {
        let next = self
            .0
            .checked_add(1)
            .ok_or_else(|| ContractError::new(ContractErrorKind::EpochExhausted, "epoch"))?;
        if next == i64::MAX {
            return Err(ContractError::new(
                ContractErrorKind::EpochExhausted,
                "epoch",
            ));
        }
        Ok(Self(next))
    }
}

macro_rules! revision_newtype {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(u64);

            impl $name {
                #[must_use]
                pub const fn new(value: u64) -> Self {
                    Self(value)
                }

                #[must_use]
                pub const fn get(self) -> u64 {
                    self.0
                }

                pub fn checked_next(self) -> Result<Self, ContractError> {
                    self.0.checked_add(1).map(Self).ok_or_else(|| {
                        ContractError::new(ContractErrorKind::EpochExhausted, stringify!($name))
                    })
                }
            }
        )+
    };
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NonZeroRevision(u64);

impl NonZeroRevision {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        if value == 0 {
            return Err(ContractError::new(
                ContractErrorKind::ZeroNotAllowed,
                "non_zero_revision",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, ContractError> {
        self.0
            .checked_add(1)
            .ok_or_else(|| ContractError::new(ContractErrorKind::EpochExhausted, "revision"))
            .and_then(Self::new)
    }
}

revision_newtype!(
    PortfolioRevision,
    CollectionRouteRevision,
    CatalogRevision,
    MembershipRevision,
    AccessPolicyRevision,
    ShadowFenceRevision,
    PurgeFenceRevision,
    ObservationCursorRevision,
    OverlayRevision,
    PolicyRevision,
);

macro_rules! digest_newtype {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name([u8; 32]);

            impl $name {
                #[must_use]
                pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                    Self(bytes)
                }

                #[must_use]
                pub const fn as_bytes(&self) -> &[u8; 32] {
                    &self.0
                }

                pub fn parse_hex(value: &str) -> Result<Self, ContractError> {
                    hex_decode(value, stringify!($name)).map(Self)
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str(&hex_encode(&self.0))
                }
            }

            impl FromStr for $name {
                type Err = ContractError;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    Self::parse_hex(value)
                }
            }
        )+
    };
}

digest_newtype!(
    Blake3Digest32,
    Sha256Digest32,
    SourceOwnerGeneration,
    ObjectResidencyKeyDigest,
    PlanFingerprint,
    QuerySnapshotFingerprint,
    ArtifactDigest,
    HandleTokenDigest,
);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DigestAlgorithm {
    Blake3_256,
    Sha256,
}

crate::impl_wire_enum!(DigestAlgorithm {
    Blake3_256 => "blake3_256",
    Sha256 => "sha256",
});

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VersionedContentDigest {
    pub algorithm: DigestAlgorithm,
    pub bytes: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DigestRef {
    Blake3(Blake3Digest32),
    Sha256(Sha256Digest32),
}

macro_rules! profile_newtype {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(BoundedText<MAX_PROFILE_ID_BYTES>);

            impl $name {
                pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
                    BoundedText::new_non_empty(value).map(Self)
                }

                #[must_use]
                pub fn as_str(&self) -> &str {
                    self.0.as_str()
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str(self.as_str())
                }
            }
        )+
    };
}

profile_newtype!(
    ProfileId,
    ProjectionProfileSetId,
    FusionProfileId,
    RecipeFamilyId
);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuleId(BoundedText<MAX_NAME_BYTES>);

impl RuleId {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        BoundedText::new_non_empty(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReceiptRef(OpaqueRef);

impl ReceiptRef {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        OpaqueRef::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Git object identity in canonical lower-case SHA-1 or SHA-256 hexadecimal.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GitObjectId(String);

impl GitObjectId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if !matches!(value.len(), 40 | 64) || value.bytes().any(|byte| lower_hex(byte).is_none()) {
            return Err(ContractError::new(
                ContractErrorKind::InvalidDigest,
                "git_object_id",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque bearer token. Debug intentionally exposes only length.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpaqueHandleToken {
    bytes: [u8; MAX_HANDLE_TOKEN_BYTES],
    len: u8,
}

impl OpaqueHandleToken {
    pub fn new(value: &[u8]) -> Result<Self, ContractError> {
        if !(MIN_HANDLE_TOKEN_BYTES..=MAX_HANDLE_TOKEN_BYTES).contains(&value.len()) {
            return Err(ContractError::new(
                ContractErrorKind::InvalidToken,
                "opaque_handle_token",
            ));
        }
        let len = u8::try_from(value.len()).map_err(|_| {
            ContractError::new(ContractErrorKind::InvalidToken, "opaque_handle_token")
        })?;
        let mut bytes = [0_u8; MAX_HANDLE_TOKEN_BYTES];
        bytes[..value.len()].copy_from_slice(value);
        Ok(Self { bytes, len })
    }

    pub fn parse_base64url(value: &str) -> Result<Self, ContractError> {
        Self::new(&base64url_decode(value)?)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    #[must_use]
    pub fn encoded(&self) -> String {
        base64url_encode(self.as_bytes())
    }
}

impl fmt::Debug for OpaqueHandleToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpaqueHandleToken")
            .field("len", &self.len)
            .field("value", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CorpusOrPortfolioId {
    Corpus(CorpusId),
    Portfolio(ReferencePortfolioId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceOrCorpusRef {
    Workspace(WorkspaceId),
    Corpus(CorpusId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceViewRef {
    pub source_view_digest: Blake3Digest32,
    pub workspace_view_revision_ref: Option<WorkspaceViewRevisionId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthorizedScopeRef {
    pub scope_domain_id: ScopeDomainId,
    pub authorized_scope_digest: Blake3Digest32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactScanPlanRef {
    pub plan_id: PlanId,
    pub plan_fingerprint: PlanFingerprint,
}

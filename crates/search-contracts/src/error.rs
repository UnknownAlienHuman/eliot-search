use core::fmt;

/// Closed P00 contract-validation namespace.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContractErrorCode {
    EpochOutOfRange,
    EpochExhausted,
    UnknownLoadBearingField,
    ContractVersionMismatch,
    InvalidContractShape,
    CanonicalizationFailed,
    DigestMismatch,
    BoundExceeded,
    InvalidTaggedVariant,
    RecipeResultMismatch,
}

impl ContractErrorCode {
    pub const ALL: [Self; 10] = [
        Self::EpochOutOfRange,
        Self::EpochExhausted,
        Self::UnknownLoadBearingField,
        Self::ContractVersionMismatch,
        Self::InvalidContractShape,
        Self::CanonicalizationFailed,
        Self::DigestMismatch,
        Self::BoundExceeded,
        Self::InvalidTaggedVariant,
        Self::RecipeResultMismatch,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EpochOutOfRange => "EPOCH_OUT_OF_RANGE",
            Self::EpochExhausted => "EPOCH_EXHAUSTED",
            Self::UnknownLoadBearingField => "UNKNOWN_LOAD_BEARING_FIELD",
            Self::ContractVersionMismatch => "CONTRACT_VERSION_MISMATCH",
            Self::InvalidContractShape => "INVALID_CONTRACT_SHAPE",
            Self::CanonicalizationFailed => "CANONICALIZATION_FAILED",
            Self::DigestMismatch => "DIGEST_MISMATCH",
            Self::BoundExceeded => "BOUND_EXCEEDED",
            Self::InvalidTaggedVariant => "INVALID_TAGGED_VARIANT",
            Self::RecipeResultMismatch => "RECIPE_RESULT_MISMATCH",
        }
    }
}

/// Precise local category. It deterministically maps to the stable public
/// contract-validation namespace without carrying unbounded input text.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContractErrorKind {
    EpochOutOfRange,
    EpochExhausted,
    UnknownField,
    UnsupportedVersion,
    MalformedPayload,
    InvalidRange,
    InvalidCharacter,
    Duplicate,
    Empty,
    ZeroNotAllowed,
    NonCanonical,
    InvalidDigest,
    DigestMismatch,
    TooLong,
    TooManyItems,
    DepthExceeded,
    OversizePayload,
    InvalidTaggedVariant,
    FamilyMismatch,
    InvalidToken,
    ForbiddenCandidateReason,
    ContradictoryState,
}

impl ContractErrorKind {
    #[must_use]
    pub const fn code(self) -> ContractErrorCode {
        match self {
            Self::EpochOutOfRange => ContractErrorCode::EpochOutOfRange,
            Self::EpochExhausted => ContractErrorCode::EpochExhausted,
            Self::UnknownField => ContractErrorCode::UnknownLoadBearingField,
            Self::UnsupportedVersion => ContractErrorCode::ContractVersionMismatch,
            Self::NonCanonical => ContractErrorCode::CanonicalizationFailed,
            Self::InvalidDigest | Self::DigestMismatch => ContractErrorCode::DigestMismatch,
            Self::TooLong | Self::TooManyItems | Self::DepthExceeded | Self::OversizePayload => {
                ContractErrorCode::BoundExceeded
            }
            Self::InvalidTaggedVariant => ContractErrorCode::InvalidTaggedVariant,
            Self::FamilyMismatch => ContractErrorCode::RecipeResultMismatch,
            Self::MalformedPayload
            | Self::InvalidRange
            | Self::InvalidCharacter
            | Self::Duplicate
            | Self::Empty
            | Self::ZeroNotAllowed
            | Self::InvalidToken
            | Self::ForbiddenCandidateReason
            | Self::ContradictoryState => ContractErrorCode::InvalidContractShape,
        }
    }
}

/// Bounded error surface containing only a static field identifier and numeric
/// cardinality. Input-controlled source/query/secret text is never retained.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContractError {
    code: ContractErrorCode,
    kind: ContractErrorKind,
    field: &'static str,
    limit: Option<u64>,
    actual: Option<u64>,
}

impl ContractError {
    #[must_use]
    pub const fn new(kind: ContractErrorKind, field: &'static str) -> Self {
        Self {
            code: kind.code(),
            kind,
            field,
            limit: None,
            actual: None,
        }
    }

    #[must_use]
    pub const fn bounded(
        kind: ContractErrorKind,
        field: &'static str,
        limit: u64,
        actual: u64,
    ) -> Self {
        Self {
            code: kind.code(),
            kind,
            field,
            limit: Some(limit),
            actual: Some(actual),
        }
    }

    #[must_use]
    pub const fn code(&self) -> ContractErrorCode {
        self.code
    }

    #[must_use]
    pub const fn kind(&self) -> ContractErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    #[must_use]
    pub const fn limit(&self) -> Option<u64> {
        self.limit
    }

    #[must_use]
    pub const fn actual(&self) -> Option<u64> {
        self.actual
    }

    #[must_use]
    pub const fn malformed(field: &'static str) -> Self {
        Self::new(ContractErrorKind::MalformedPayload, field)
    }

    #[must_use]
    pub const fn unsupported_version(field: &'static str) -> Self {
        Self::new(ContractErrorKind::UnsupportedVersion, field)
    }

    #[must_use]
    pub const fn unknown_field(field: &'static str) -> Self {
        Self::new(ContractErrorKind::UnknownField, field)
    }

    #[must_use]
    pub const fn invalid_variant(field: &'static str) -> Self {
        Self::new(ContractErrorKind::InvalidTaggedVariant, field)
    }

    #[must_use]
    pub fn oversize(field: &'static str, limit: usize, actual: usize) -> Self {
        Self::bounded(
            ContractErrorKind::OversizePayload,
            field,
            u64::try_from(limit).unwrap_or(u64::MAX),
            u64::try_from(actual).unwrap_or(u64::MAX),
        )
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.code.as_str(), self.field)?;
        if let (Some(limit), Some(actual)) = (self.limit, self.actual) {
            write!(formatter, " (limit {limit}, actual {actual})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ContractError {}

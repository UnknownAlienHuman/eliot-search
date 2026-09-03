//! Closed, content-free configuration failures.

use std::fmt;

/// Deterministic configuration failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConfigError {
    /// The configured finite limit set is internally inconsistent.
    InvalidLimits,
    /// Input is empty where a non-empty value is required.
    EmptyInput,
    /// Input exceeds a finite byte, item, or nesting limit.
    CapacityExceeded,
    /// A UTF-8 byte order mark is forbidden.
    BomForbidden,
    /// Input is not valid UTF-8.
    InvalidUtf8,
    /// The bounded parser does not accept the encountered TOML construct.
    UnsupportedSyntax,
    /// The configuration schema version is absent, malformed, or unsupported.
    UnsupportedSchemaVersion,
    /// A table is declared more than once.
    DuplicateTable,
    /// A key is declared more than once in one document or layer.
    DuplicateKey,
    /// A canonical section, key, path, owner, profile, or source name is invalid.
    InvalidIdentifier,
    /// A section is not present in the closed registry.
    UnknownSection,
    /// A key is not present in its registered section.
    UnknownKey,
    /// Two section descriptors claim the same section name.
    SectionConflict,
    /// Two field descriptors claim the same field path.
    FieldConflict,
    /// A descriptor is internally inconsistent.
    InvalidDescriptor,
    /// A document source kind does not match its assigned layer.
    SourceKindMismatch,
    /// The same precedence source was supplied more than once.
    DuplicateSource,
    /// A source is not allowed to override this field.
    OverrideNotAllowed,
    /// An explicit reset is not allowed for this field.
    ResetNotAllowed,
    /// The supplied value kind differs from the field descriptor.
    TypeMismatch,
    /// A value violates its finite field bounds.
    ValueOutOfBounds,
    /// A plaintext value was supplied where only an opaque secret reference is legal.
    SecretPlaintextForbidden,
    /// A change weakens a fixed or directional security floor.
    SecurityFloorViolation,
    /// A required validated section is missing.
    MissingSection,
    /// A validated section was supplied more than once.
    DuplicateValidatedSection,
    /// A section result is bound to a stale descriptor revision or digest.
    StaleDescriptor,
    /// A section result is bound to another selected profile.
    ProfileNotAuthorized,
    /// Canonical fingerprint input exceeds its finite byte ceiling.
    CanonicalBytesExceeded,
    /// A length conversion or accumulation overflowed.
    LengthOverflow,
    /// A prefixed environment key is malformed or not registered.
    InvalidEnvironmentKey,
    /// The change set includes an explicit reject obligation.
    ReconfigurationRejected,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "configuration limits are invalid",
            Self::EmptyInput => "configuration input is empty",
            Self::CapacityExceeded => "configuration capacity exceeded",
            Self::BomForbidden => "UTF-8 byte order mark is forbidden",
            Self::InvalidUtf8 => "configuration input is not valid UTF-8",
            Self::UnsupportedSyntax => "configuration syntax is unsupported",
            Self::UnsupportedSchemaVersion => "configuration schema version is unsupported",
            Self::DuplicateTable => "configuration table is duplicated",
            Self::DuplicateKey => "configuration key is duplicated",
            Self::InvalidIdentifier => "configuration identifier is invalid",
            Self::UnknownSection => "configuration section is unknown",
            Self::UnknownKey => "configuration key is unknown",
            Self::SectionConflict => "configuration section ownership conflicts",
            Self::FieldConflict => "configuration field ownership conflicts",
            Self::InvalidDescriptor => "configuration descriptor is invalid",
            Self::SourceKindMismatch => "configuration source kind does not match its layer",
            Self::DuplicateSource => "configuration source is duplicated",
            Self::OverrideNotAllowed => "configuration override source is not allowed",
            Self::ResetNotAllowed => "configuration reset is not allowed",
            Self::TypeMismatch => "configuration value type mismatch",
            Self::ValueOutOfBounds => "configuration value exceeds its finite bounds",
            Self::SecretPlaintextForbidden => "plaintext secret configuration is forbidden",
            Self::SecurityFloorViolation => "configuration change weakens a security floor",
            Self::MissingSection => "validated configuration section is missing",
            Self::DuplicateValidatedSection => "validated configuration section is duplicated",
            Self::StaleDescriptor => "validated configuration descriptor is stale",
            Self::ProfileNotAuthorized => "configuration profile is not authorized",
            Self::CanonicalBytesExceeded => "canonical configuration bytes exceed their limit",
            Self::LengthOverflow => "configuration length overflow",
            Self::InvalidEnvironmentKey => "environment configuration key is invalid",
            Self::ReconfigurationRejected => "configuration change is explicitly rejected",
        })
    }
}

impl std::error::Error for ConfigError {}

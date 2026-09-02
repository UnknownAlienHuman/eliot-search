//! Bounded package-local errors.

use core::fmt;
use search_contracts::{ContractError, ContractErrorCode};

/// Closed semantic failure class owned by `search-domain`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DomainErrorKind {
    /// A contract value failed its own closed shape validation.
    InvalidContract,
    /// A closed state machine rejected a skipped, reverse, or dual-owner edge.
    InvalidStateTransition,
    /// A load-bearing invariant was contradicted.
    InvariantViolation,
    /// A supplied BLAKE3-256 fingerprint did not match canonical input bytes.
    FingerprintMismatch,
    /// Retrieval and IDF eligibility predicates were not exactly equivalent.
    EligibilityFilterMismatch,
}

/// Bounded domain failure carrying no source/query/secret content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainError {
    kind: DomainErrorKind,
    contract_code: Option<ContractErrorCode>,
    field: &'static str,
}

impl DomainError {
    /// Creates a package-local semantic error.
    #[must_use]
    pub const fn new(kind: DomainErrorKind, field: &'static str) -> Self {
        Self {
            kind,
            contract_code: None,
            field,
        }
    }

    /// Creates a bounded wrapper for a contract-validation error.
    #[must_use]
    pub const fn contract(error: &ContractError) -> Self {
        Self {
            kind: DomainErrorKind::InvalidContract,
            contract_code: Some(error.code()),
            field: error.field(),
        }
    }

    /// Semantic error class.
    #[must_use]
    pub const fn kind(&self) -> DomainErrorKind {
        self.kind
    }

    /// Stable underlying contract code, when shape validation failed.
    #[must_use]
    pub const fn contract_code(&self) -> Option<ContractErrorCode> {
        self.contract_code
    }

    /// Static field or invariant identifier.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }
}

impl From<ContractError> for DomainError {
    fn from(error: ContractError) -> Self {
        Self::contract(&error)
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {}", self.kind, self.field)
    }
}

impl std::error::Error for DomainError {}

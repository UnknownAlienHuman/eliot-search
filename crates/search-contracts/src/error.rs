use core::fmt;

/// Public category for canonical source-identity parsing failures.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContractErrorCode {
    InvalidContractShape,
}

impl ContractErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidContractShape => "INVALID_CONTRACT_SHAPE",
        }
    }
}

/// Content-free source-identity parsing cause.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContractErrorKind {
    MalformedPayload,
    InvalidCharacter,
}

/// A static field identifier and parsing cause; no input text is retained.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContractError {
    kind: ContractErrorKind,
    field: &'static str,
}

impl ContractError {
    #[must_use]
    pub const fn new(kind: ContractErrorKind, field: &'static str) -> Self {
        Self { kind, field }
    }

    #[must_use]
    pub const fn code(&self) -> ContractErrorCode {
        ContractErrorCode::InvalidContractShape
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
    pub const fn malformed(field: &'static str) -> Self {
        Self::new(ContractErrorKind::MalformedPayload, field)
    }
}

impl fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.code().as_str(), self.field)
    }
}

impl std::error::Error for ContractError {}

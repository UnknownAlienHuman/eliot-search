//! Loopback and process-containment result types.
//!
//! Production containment is created only by the private Windows adapter.
//! Public callers cannot manufacture an ACL or Job Object attestation.

use crate::SupervisorError;

/// Verified containment result attached to a process guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContainmentReport {
    contained: bool,
}

impl ContainmentReport {
    /// Whether the process was born in a kill-on-close Job Object and its
    /// owner data root was protected by a verified owner-only ACL.
    #[must_use]
    pub const fn is_contained(self) -> bool {
        self.contained
    }

    pub(crate) const fn windows_verified() -> Self {
        Self { contained: true }
    }

    pub(crate) const fn unverified() -> Self {
        Self { contained: false }
    }
}

/// Loopback-only bind host. Only exact loopback literals are admitted;
/// `localhost` is refused because its resolution is machine-dependent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopbackHost {
    /// `127.0.0.1`.
    V4,
    /// `::1`.
    V6,
}

impl LoopbackHost {
    /// Renders the literal used for sockets and the Qdrant config file.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V4 => "127.0.0.1",
            Self::V6 => "::1",
        }
    }
}

/// Parses and admits only exact loopback literals.
pub fn parse_loopback_host(value: &str) -> Result<LoopbackHost, SupervisorError> {
    match value {
        "127.0.0.1" => Ok(LoopbackHost::V4),
        "::1" => Ok(LoopbackHost::V6),
        _ => Err(SupervisorError::NonLoopbackEndpoint),
    }
}

#[cfg(test)]
mod tests {
    use super::{LoopbackHost, parse_loopback_host};
    use crate::SupervisorError;

    #[test]
    fn only_exact_loopback_literals_are_admitted() {
        assert_eq!(parse_loopback_host("127.0.0.1").unwrap(), LoopbackHost::V4);
        assert_eq!(parse_loopback_host("::1").unwrap(), LoopbackHost::V6);
        for rejected in [
            "0.0.0.0",
            "::",
            "localhost",
            "127.0.0.2",
            "10.0.0.1",
            "",
            "127.1",
        ] {
            assert_eq!(
                parse_loopback_host(rejected).unwrap_err(),
                SupervisorError::NonLoopbackEndpoint,
                "{rejected:?} must be rejected"
            );
        }
    }
}

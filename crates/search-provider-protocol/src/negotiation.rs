//! Exact major/minor hello negotiation over canonical protocol ranges.
//!
//! Major mismatch fails. Minor/extension negotiation is explicit and cannot
//! reinterpret load-bearing fields. Capability availability grants no
//! authority. This package re-exports the canonical `ProtocolVersion`,
//! `ProtocolRange` and `RequestId` types instead of defining duplicates
//! (norm #89).

use search_contracts::{ContractErrorKind, ProtocolRange, ProtocolVersion};

/// Canonical transport payload owned by `search-contracts`.
pub use search_contracts::protocol::{JsonFramePayload, ProviderEnvelope};

use crate::error::ProtocolError;

/// Selects the highest mutually supported `(major, minor)` version.
///
/// Major mismatch fails. Minor negotiation is explicit and cannot reinterpret
/// load-bearing fields. Delegates to the canonical
/// `search-contracts::protocol::ProtocolRange::negotiate`.
pub fn negotiate_hello(
    local: ProtocolRange,
    remote: ProtocolRange,
) -> Result<ProtocolVersion, ProtocolError> {
    local
        .negotiate(remote)
        .map_err(|_| ProtocolError::NoCompatibleVersion)
}

/// Alias for [`negotiate_hello`] preserved for intra-package callers.
pub fn negotiate_version(
    client: ProtocolRange,
    provider: ProtocolRange,
) -> Result<ProtocolVersion, ProtocolError> {
    negotiate_hello(client, provider)
}

/// Validates the envelope tag and requires its version to be in the
/// negotiated canonical range.
pub fn validate_envelope_version(
    envelope: &ProviderEnvelope,
    supported: ProtocolRange,
) -> Result<(), ProtocolError> {
    envelope
        .validate_version_and_limits(supported)
        .map_err(|error| {
            if error.kind() == ContractErrorKind::UnsupportedVersion {
                ProtocolError::NoCompatibleVersion
            } else {
                ProtocolError::InvalidEnvelope
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u16, minor: u16) -> ProtocolVersion {
        ProtocolVersion { major, minor }
    }

    fn range(min_major: u16, min_minor: u16, max_major: u16, max_minor: u16) -> ProtocolRange {
        ProtocolRange::new(version(min_major, min_minor), version(max_major, max_minor))
            .expect("range")
    }

    #[test]
    fn negotiation_selects_highest_minor_and_rejects_major_mismatch() {
        let selected = negotiate_hello(range(1, 0, 1, 2), range(1, 1, 1, 5)).expect("overlap");
        assert_eq!(selected, version(1, 2));
        assert_eq!(
            negotiate_hello(range(1, 0, 1, 1), range(2, 0, 2, 0)),
            Err(ProtocolError::NoCompatibleVersion)
        );
        assert_eq!(
            negotiate_hello(range(1, 0, 1, 1), range(1, 2, 1, 3)),
            Err(ProtocolError::NoCompatibleVersion)
        );
    }

    #[test]
    fn canonical_types_are_not_duplicated() {
        // `RequestId` is re-exported from the crate root; it must be the
        // canonical contracts type.
        assert_eq!(
            std::any::type_name::<crate::RequestId>(),
            std::any::type_name::<search_contracts::RequestId>()
        );
        assert_eq!(
            std::any::type_name::<ProtocolVersion>(),
            std::any::type_name::<search_contracts::protocol::ProtocolVersion>()
        );
        assert_eq!(
            std::any::type_name::<ProtocolRange>(),
            std::any::type_name::<search_contracts::protocol::ProtocolRange>()
        );
    }
}

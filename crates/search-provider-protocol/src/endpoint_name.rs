//! Canonical installation-scoped names for the local provider transport.
//!
//! This module owns only the bounded protocol name. Operating-system adapters
//! decide how to map it into a local IPC namespace. In particular, callers
//! must not prepend a filesystem path, host, IP address, port, remote pipe
//! prefix or user-controlled suffix before authentication.

use core::{fmt, str::FromStr};

use search_contracts::InstallationId;

use crate::error::ProtocolError;

const NATIVE_ENDPOINT_NAME_PREFIX: &str = "eliot-search-v1-";
const CANONICAL_UUID_TEXT_BYTES: usize = 36;

/// Exact UTF-8 byte length of [`NativeEndpointNameV1`].
///
/// The value is fixed: a 16-byte ASCII prefix followed by one canonical
/// lower-case UUID. It is a protocol ceiling and not an invitation to append
/// transport-specific coordinates.
pub const NATIVE_ENDPOINT_NAME_BYTES: usize =
    NATIVE_ENDPOINT_NAME_PREFIX.len() + CANONICAL_UUID_TEXT_BYTES;

/// Canonical name of one installation's local provider endpoint.
///
/// The value is derived only from an independently trusted [`InstallationId`].
/// It contains no path, host, IP address, port, user name, secret, credential
/// locator or authority. Pipe ACLs and successful name resolution remain
/// insufficient authentication; the normal pairing and binding ceremony is
/// still mandatory.
///
/// Canonical text is exactly:
///
/// ```text
/// eliot-search-v1-00112233-4455-6677-8899-aabbccddeeff
/// ```
///
/// where the UUID portion is the installation identity. Parsing rejects
/// alternate case, separators, prefixes, suffixes and non-canonical UUID text.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeEndpointNameV1 {
    installation_id: InstallationId,
}

impl NativeEndpointNameV1 {
    /// Derive the canonical endpoint name for one installation.
    #[must_use]
    pub const fn from_installation(installation_id: InstallationId) -> Self {
        Self { installation_id }
    }

    /// Installation identity encoded by this endpoint name.
    #[must_use]
    pub const fn installation_id(&self) -> InstallationId {
        self.installation_id
    }

    /// Parse one exact canonical endpoint name.
    ///
    /// Filesystem paths, Windows pipe namespace prefixes, remote machine
    /// prefixes, IP/port coordinates, alternate UUID spelling and trailing
    /// bytes all fail closed as [`ProtocolError::InvalidBody`].
    pub fn parse(value: &str) -> Result<Self, ProtocolError> {
        if value.len() != NATIVE_ENDPOINT_NAME_BYTES {
            return Err(ProtocolError::InvalidBody);
        }
        let installation = value
            .strip_prefix(NATIVE_ENDPOINT_NAME_PREFIX)
            .ok_or(ProtocolError::InvalidBody)?;
        let installation_id =
            InstallationId::parse(installation).map_err(|_| ProtocolError::InvalidBody)?;
        Ok(Self { installation_id })
    }
}

impl From<InstallationId> for NativeEndpointNameV1 {
    fn from(installation_id: InstallationId) -> Self {
        Self::from_installation(installation_id)
    }
}

impl FromStr for NativeEndpointNameV1 {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for NativeEndpointNameV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{NATIVE_ENDPOINT_NAME_PREFIX}{}",
            self.installation_id
        )
    }
}

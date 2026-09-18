//! Finite bindings, limits and failures for the legacy revision envelope.

use core::fmt;

/// Legacy protected revision filename extension.
pub const LEGACY_REVISION_PROTECTED_OBJECT_EXTENSION: &str = "dpapi";
/// Frozen protected-object envelope version.
pub const LEGACY_REVISION_OBJECT_VERSION: u32 = 1;
/// Maximum admitted plaintext bytes for the compatibility format.
pub const LEGACY_REVISION_MAX_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024;
/// Maximum admitted protected-object bytes for the compatibility format.
pub const LEGACY_REVISION_MAX_PROTECTED_OBJECT_BYTES: usize = 65 * 1024 * 1024;
/// Maximum admitted plaintext length represented by the wire binding.
pub const LEGACY_REVISION_MAX_PLAINTEXT_LENGTH: u64 = 64 * 1024 * 1024;
/// Exact outer header size before protected payload bytes.
pub const LEGACY_REVISION_OUTER_HEADER_BYTES: usize =
    8 + 4 + 32 + 32 + 32 + 32 + 8 + 8;
/// Exact authenticated inner header size before plaintext bytes.
pub const LEGACY_REVISION_INNER_HEADER_BYTES: usize =
    8 + 4 + 32 + 32 + 32 + 32 + 8;

/// Injected digest owner for validating exact plaintext bytes.
pub trait LegacyRevisionContentDigest {
    /// Returns the exact 32-byte digest of `bytes`.
    fn digest(bytes: &[u8]) -> [u8; 32];
}

/// Exact source-revision identity authenticated by both envelope layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyRevisionExpected {
    namespace_id: [u8; 32],
    revision_id: [u8; 32],
    content_digest: [u8; 32],
    plaintext_len: u64,
}

impl LegacyRevisionExpected {
    /// Creates one exact expected source-revision identity.
    #[must_use]
    pub const fn new(
        namespace_id: [u8; 32],
        revision_id: [u8; 32],
        content_digest: [u8; 32],
        plaintext_len: u64,
    ) -> Self {
        Self {
            namespace_id,
            revision_id,
            content_digest,
            plaintext_len,
        }
    }

    /// Bound namespace identity.
    #[must_use]
    pub const fn namespace_id(self) -> [u8; 32] {
        self.namespace_id
    }

    /// Bound revision identity.
    #[must_use]
    pub const fn revision_id(self) -> [u8; 32] {
        self.revision_id
    }

    /// Bound plaintext content digest.
    #[must_use]
    pub const fn content_digest(self) -> [u8; 32] {
        self.content_digest
    }

    /// Exact bound plaintext length.
    #[must_use]
    pub const fn plaintext_len(self) -> u64 {
        self.plaintext_len
    }
}

/// Complete legacy envelope binding, including platform-key identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyRevisionBinding {
    revision: LegacyRevisionExpected,
    key_binding_digest: [u8; 32],
}

impl LegacyRevisionBinding {
    /// Creates one complete immutable envelope binding.
    #[must_use]
    pub const fn new(
        revision: LegacyRevisionExpected,
        key_binding_digest: [u8; 32],
    ) -> Self {
        Self {
            revision,
            key_binding_digest,
        }
    }

    /// Exact source-revision identity.
    #[must_use]
    pub const fn revision(self) -> LegacyRevisionExpected {
        self.revision
    }

    /// Digest binding the envelope to one platform key.
    #[must_use]
    pub const fn key_binding_digest(self) -> [u8; 32] {
        self.key_binding_digest
    }
}

/// Closed pure failure from the legacy envelope contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyRevisionEnvelopeError {
    /// Protected payload is empty or would exceed the object ceiling.
    ProtectedPayloadInvalid,
    /// Requested plaintext exceeds the compatibility ceiling.
    PlaintextTooLarge,
    /// Input is not marked as a protected legacy object.
    ProtectedFormatRequired,
    /// Outer framing, version, or payload length is malformed.
    EnvelopeInvalid,
    /// Outer namespace differs from the exact expected namespace.
    NamespaceMismatch,
    /// Outer key binding differs from the expected platform-key binding.
    KeyBindingMismatch,
    /// Other outer source-revision binding fields differ.
    EnvelopeBindingMismatch,
    /// Authenticated inner framing or version is malformed.
    InnerEnvelopeInvalid,
    /// Authenticated inner identity differs from the outer binding.
    InnerBindingMismatch,
    /// Plaintext length differs from the bound length.
    LengthMismatch,
    /// Plaintext digest differs from the bound content digest.
    ContentMismatch,
}

impl LegacyRevisionEnvelopeError {
    /// Stable package-local reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ProtectedPayloadInvalid => {
                "SECRET_LEGACY_REVISION_PROTECTED_PAYLOAD_INVALID"
            }
            Self::PlaintextTooLarge => {
                "SECRET_LEGACY_REVISION_PLAINTEXT_TOO_LARGE"
            }
            Self::ProtectedFormatRequired => {
                "SECRET_LEGACY_REVISION_PROTECTED_FORMAT_REQUIRED"
            }
            Self::EnvelopeInvalid => {
                "SECRET_LEGACY_REVISION_ENVELOPE_INVALID"
            }
            Self::NamespaceMismatch => {
                "SECRET_LEGACY_REVISION_NAMESPACE_MISMATCH"
            }
            Self::KeyBindingMismatch => {
                "SECRET_LEGACY_REVISION_KEY_BINDING_MISMATCH"
            }
            Self::EnvelopeBindingMismatch => {
                "SECRET_LEGACY_REVISION_BINDING_MISMATCH"
            }
            Self::InnerEnvelopeInvalid => {
                "SECRET_LEGACY_REVISION_INNER_ENVELOPE_INVALID"
            }
            Self::InnerBindingMismatch => {
                "SECRET_LEGACY_REVISION_INNER_BINDING_MISMATCH"
            }
            Self::LengthMismatch => {
                "SECRET_LEGACY_REVISION_LENGTH_MISMATCH"
            }
            Self::ContentMismatch => {
                "SECRET_LEGACY_REVISION_CONTENT_MISMATCH"
            }
        }
    }
}

impl fmt::Display for LegacyRevisionEnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyRevisionEnvelopeError {}

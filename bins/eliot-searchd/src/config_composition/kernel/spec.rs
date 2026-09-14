//! Closed daemon configuration constants, identifiers, digests and error codes.

use search_config::{
    ConfigError, ConfigKeyName, ConfigLimits, ConfigOwner, ConfigSectionName,
    ConfigSource, ConfigSourceKind, ConfigSourceRef, ValueBounds,
};
use search_contracts::{Blake3Digest32, ProfileId};

/// Exact configuration schema version from `config/sections.toml`.
pub const DAEMON_CONFIG_SCHEMA_VERSION: u32 = 1;
/// Externally authorized W1 shell profile.
pub const DAEMON_DIRECT_PROFILE: &str = "direct";
/// Bounded captured inputs for one composition attempt.
pub const MAX_CAPTURED_FILE_BYTES: usize = 64 * 1024;
/// Maximum captured environment or CLI entries for one composition attempt.
pub const MAX_CAPTURED_ENTRIES: usize = 256;
/// Maximum captured value bytes before typed validation.
pub const MAX_CAPTURED_VALUE_BYTES: usize = 4_096;
/// Explicit reset marker accepted only where the descriptor allows reset.
pub const RESET_MARKER: &str = "__RESET__";

/// Candidate cannot be activated because configuration orchestration failed.
pub const ACTIVATION_BLOCKED: &str = "DAEMON_CONFIG_ACTIVATION_BLOCKED";
/// Candidate explicitly rejected by a field or section obligation.
pub const ACTIVATION_REJECTED: &str = "DAEMON_CONFIG_REJECTED";
/// Mixed obligations without every required receipt; old snapshot retained.
pub const ACTIVATION_PARTIAL_REFUSED: &str = "DAEMON_CONFIG_PARTIAL_REFUSED";
/// Optional profile requested without an external gate receipt.
pub const OPTIONAL_GATE_REQUIRED: &str = "DAEMON_CONFIG_GATE_REQUIRED";
/// General search requested without an accepted search receipt.
pub const SEARCH_NOT_ACCEPTED: &str = "DAEMON_SEARCH_NOT_ACCEPTED";
/// Indexed search requested without accepted indexed receipts.
pub const INDEXED_NOT_ACCEPTED: &str = "DAEMON_INDEXED_NOT_ACCEPTED";
/// Control dependency is not verified.
pub const CONTROL_NOT_READY: &str = "DAEMON_CONTROL_NOT_READY";
/// Direct store dependency is not verified.
pub const DIRECT_NOT_READY: &str = "DAEMON_DIRECT_NOT_READY";
/// Persistent quarantine is armed.
pub const QUARANTINED_BLOCKER: &str = "DAEMON_QUARANTINED";

pub(super) const fn daemon_limits() -> ConfigLimits {
    ConfigLimits::W1
}

pub(super) const fn const_bounds() -> ValueBounds {
    ValueBounds {
        max_text_bytes: MAX_CAPTURED_VALUE_BYTES,
        max_list_items: 64,
        max_list_item_bytes: 512,
        integer_min: 0,
        integer_max: 16_777_216,
    }
}

pub(super) fn profile_id(value: &str) -> Result<ProfileId, ConfigError> {
    ProfileId::new(value).map_err(|_| ConfigError::InvalidIdentifier)
}

pub(super) fn section_name(value: &str) -> Result<ConfigSectionName, ConfigError> {
    ConfigSectionName::new(value, 128)
}

pub(super) fn key_name(value: &str) -> Result<ConfigKeyName, ConfigError> {
    ConfigKeyName::new(value, 128)
}

pub(super) fn owner_name(value: &str) -> Result<ConfigOwner, ConfigError> {
    ConfigOwner::new(value, 128)
}

pub(super) fn source_ref(value: &str) -> Result<ConfigSourceRef, ConfigError> {
    ConfigSourceRef::new(value, 128)
}

pub(super) fn blake3_digest(bytes: &[u8]) -> Blake3Digest32 {
    Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes())
}

pub(super) fn defaults_source() -> Result<ConfigSource, ConfigError> {
    Ok(ConfigSource {
        kind: ConfigSourceKind::Defaults,
        source_ref: source_ref("daemon-compiled-defaults")?,
        source_digest: blake3_digest(b"eliot-searchd/compiled-defaults/v1"),
    })
}

/// Maps a pure configuration failure to a closed daemon code without
/// disclosing values, paths, or secrets.
#[must_use]
pub const fn config_code(error: ConfigError) -> &'static str {
    match error {
        ConfigError::SecretPlaintextForbidden => {
            "DAEMON_CONFIG_SECRET_PLAINTEXT_FORBIDDEN"
        }
        ConfigError::SecurityFloorViolation => {
            "DAEMON_CONFIG_SECURITY_FLOOR_VIOLATION"
        }
        ConfigError::UnknownSection => "DAEMON_CONFIG_UNKNOWN_SECTION",
        ConfigError::UnknownKey => "DAEMON_CONFIG_UNKNOWN_KEY",
        ConfigError::DuplicateKey
        | ConfigError::DuplicateTable
        | ConfigError::DuplicateSource => "DAEMON_CONFIG_DUPLICATE",
        ConfigError::DuplicateValidatedSection | ConfigError::MissingSection => {
            "DAEMON_CONFIG_INCOMPLETE"
        }
        ConfigError::OverrideNotAllowed => "DAEMON_CONFIG_OVERRIDE_NOT_ALLOWED",
        ConfigError::ResetNotAllowed => "DAEMON_CONFIG_RESET_NOT_ALLOWED",
        ConfigError::TypeMismatch => "DAEMON_CONFIG_TYPE_MISMATCH",
        ConfigError::ValueOutOfBounds | ConfigError::CapacityExceeded => {
            "DAEMON_CONFIG_BOUNDS_EXCEEDED"
        }
        ConfigError::ProfileNotAuthorized => {
            "DAEMON_CONFIG_PROFILE_NOT_AUTHORIZED"
        }
        ConfigError::StaleDescriptor => "DAEMON_CONFIG_STALE_DESCRIPTOR",
        ConfigError::ReconfigurationRejected => ACTIVATION_REJECTED,
        ConfigError::InvalidEnvironmentKey => {
            "DAEMON_CONFIG_INVALID_ENVIRONMENT_KEY"
        }
        _ => "DAEMON_CONFIG_INVALID",
    }
}

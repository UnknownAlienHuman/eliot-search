//! Daemon composition for the package-owned Windows root-secret lifecycle.

use std::path::Path;

use search_os_secrets_windows::{
    LegacyRevisionRootSecret, LegacyRevisionRootSecretError,
    LegacyRevisionRootSecretRequirement,
    load_existing_legacy_revision_root_secret,
    load_or_create_legacy_revision_root_secret,
};

use super::inventory::contains_protected_objects;

pub(super) fn load_or_create_root_secret(
    namespace_id: [u8; 32],
    revision_root: &Path,
) -> Result<LegacyRevisionRootSecret, String> {
    if let Some(secret) = load_existing_root_secret(namespace_id)? {
        return Ok(secret);
    }
    let requirement = if contains_protected_objects(revision_root)? {
        LegacyRevisionRootSecretRequirement::RequireExisting
    } else {
        LegacyRevisionRootSecretRequirement::CreateIfMissing
    };
    load_or_create_legacy_revision_root_secret(&namespace_id, requirement)
        .map_err(root_secret_reason)
}

pub(super) fn load_existing_root_secret(
    namespace_id: [u8; 32],
) -> Result<Option<LegacyRevisionRootSecret>, String> {
    load_existing_legacy_revision_root_secret(&namespace_id)
        .map_err(root_secret_reason)
}

fn root_secret_reason(error: LegacyRevisionRootSecretError) -> String {
    match error {
        LegacyRevisionRootSecretError::UnsupportedPlatform => {
            "DIRECT_REVISION_ENCRYPTION_UNAVAILABLE".to_owned()
        }
        LegacyRevisionRootSecretError::CredentialReadFailed(code) => {
            format!("DIRECT_REVISION_KEY_READ_FAILED:{code}")
        }
        LegacyRevisionRootSecretError::CredentialReadbackInvalid => {
            "DIRECT_REVISION_KEY_READBACK_INVALID".to_owned()
        }
        LegacyRevisionRootSecretError::CredentialWriteFailed(code) => {
            format!("DIRECT_REVISION_KEY_WRITE_FAILED:{code}")
        }
        LegacyRevisionRootSecretError::CredentialTooLarge => {
            "DIRECT_REVISION_KEY_TOO_LARGE".to_owned()
        }
        LegacyRevisionRootSecretError::CredentialReadbackMismatch => {
            "DIRECT_REVISION_KEY_READBACK_MISMATCH".to_owned()
        }
        LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown => {
            "DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN".to_owned()
        }
        LegacyRevisionRootSecretError::MissingExistingCredential => {
            "DIRECT_REVISION_KEY_MISSING".to_owned()
        }
        LegacyRevisionRootSecretError::RandomGenerationFailed(status) => {
            format!("DIRECT_REVISION_RNG_FAILED:{status}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::root_secret_reason;
    use search_os_secrets_windows::LegacyRevisionRootSecretError;

    #[test]
    fn package_failures_preserve_direct_reason_contract() {
        assert_eq!(
            root_secret_reason(
                LegacyRevisionRootSecretError::CredentialReadFailed(5),
            ),
            "DIRECT_REVISION_KEY_READ_FAILED:5"
        );
        assert_eq!(
            root_secret_reason(
                LegacyRevisionRootSecretError::CredentialWriteFailed(13),
            ),
            "DIRECT_REVISION_KEY_WRITE_FAILED:13"
        );
        assert_eq!(
            root_secret_reason(
                LegacyRevisionRootSecretError::RandomGenerationFailed(-7),
            ),
            "DIRECT_REVISION_RNG_FAILED:-7"
        );
        assert_eq!(
            root_secret_reason(
                LegacyRevisionRootSecretError::MissingExistingCredential,
            ),
            "DIRECT_REVISION_KEY_MISSING"
        );
        assert_eq!(
            root_secret_reason(
                LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown,
            ),
            "DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN"
        );
    }
}

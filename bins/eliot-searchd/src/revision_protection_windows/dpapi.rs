//! DIRECT compatibility translation around the platform DPAPI owner.

use search_os_secrets_windows::{
    ContextualLegacyRevisionDpapiError, LegacyRevisionDpapiContext,
    LegacyRevisionDpapiError, protect_legacy_revision_current_user,
    unprotect_legacy_revision_current_user, unprotect_legacy_revision_current_user_with_context,
};
use crate::direct_store::migration_context::MigrationDpapiContextFailure;

pub(in crate::revision_protection) fn protect_data(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    protect_legacy_revision_current_user(input, entropy).map_err(direct_reason)
}

pub(in crate::revision_protection) fn unprotect_data(
    input: &mut [u8],
    entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    unprotect_legacy_revision_current_user(input, entropy).map_err(direct_reason)
}

pub(in crate::revision_protection) fn unprotect_data_with_context(
    context: &mut impl LegacyRevisionDpapiContext<Error = MigrationDpapiContextFailure>,
    input: &mut [u8], entropy: &[u8; 32],
) -> Result<Vec<u8>, String> {
    unprotect_legacy_revision_current_user_with_context(context, input, entropy)
        .map_err(contextual_reason)
}

fn contextual_reason(
    error: ContextualLegacyRevisionDpapiError<MigrationDpapiContextFailure>,
) -> String {
    match error {
        ContextualLegacyRevisionDpapiError::Context(context) => context.public_reason().to_owned(),
        ContextualLegacyRevisionDpapiError::Native(error) => direct_reason(error),
        ContextualLegacyRevisionDpapiError::NativeAndContext { context, .. } => {
            if context.clear_accounting_failed() {
                "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT_CLEAR_ACCOUNTING_FAILED".to_owned()
            } else { "DPAPI_LEGACY_REVISION_NATIVE_AND_CONTEXT".to_owned() }
        }
        ContextualLegacyRevisionDpapiError::Allocation(_) => {
            "DPAPI_LEGACY_REVISION_ALLOCATION_FAILED".to_owned()
        }
        ContextualLegacyRevisionDpapiError::AllocationAndContext { context, .. } => {
            if context.clear_accounting_failed() {
                "DPAPI_LEGACY_REVISION_ALLOCATION_AND_CONTEXT_CLEAR_ACCOUNTING_FAILED".to_owned()
            } else { "DPAPI_LEGACY_REVISION_ALLOCATION_AND_CONTEXT".to_owned() }
        }
    }
}

fn direct_reason(error: LegacyRevisionDpapiError) -> String {
    match error {
        LegacyRevisionDpapiError::UnsupportedPlatform => {
            "DIRECT_REVISION_ENCRYPTION_UNAVAILABLE".to_owned()
        }
        LegacyRevisionDpapiError::InputTooLarge => {
            "DIRECT_DPAPI_INPUT_TOO_LARGE".to_owned()
        }
        LegacyRevisionDpapiError::OutputTooLarge => {
            "DIRECT_DPAPI_OUTPUT_TOO_LARGE".to_owned()
        }
        LegacyRevisionDpapiError::InvalidPlatformOutput => {
            "DIRECT_DPAPI_OUTPUT_INVALID".to_owned()
        }
        LegacyRevisionDpapiError::ProtectFailed(code) => {
            format!("DIRECT_DPAPI_PROTECT_FAILED:{code}")
        }
        LegacyRevisionDpapiError::UnprotectFailed(code) => {
            format!("DIRECT_DPAPI_UNPROTECT_FAILED:{code}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_failures_preserve_direct_compatibility_reasons() {
        assert_eq!(
            direct_reason(LegacyRevisionDpapiError::InputTooLarge),
            "DIRECT_DPAPI_INPUT_TOO_LARGE"
        );
        assert_eq!(
            direct_reason(LegacyRevisionDpapiError::OutputTooLarge),
            "DIRECT_DPAPI_OUTPUT_TOO_LARGE"
        );
        assert_eq!(
            direct_reason(LegacyRevisionDpapiError::InvalidPlatformOutput),
            "DIRECT_DPAPI_OUTPUT_INVALID"
        );
        assert_eq!(
            direct_reason(LegacyRevisionDpapiError::ProtectFailed(5)),
            "DIRECT_DPAPI_PROTECT_FAILED:5"
        );
        assert_eq!(
            direct_reason(LegacyRevisionDpapiError::UnprotectFailed(13)),
            "DIRECT_DPAPI_UNPROTECT_FAILED:13"
        );
    }
}

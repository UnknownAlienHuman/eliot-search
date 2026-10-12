//! Contextual initialization/existing-key protection; legacy constructors remain unchanged.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use super::RevisionProtector;
use crate::owner_composition::InitializingDataRoot;
use search_runtime_owner::OwnerError;

impl RevisionProtector {
    pub(crate) fn for_initialization(
        root: &mut InitializingDataRoot,
        create: bool,
    ) -> Result<Self, OwnerError> {
        #[cfg(windows)]
        {
            use super::super::envelope::DirectRevisionDigest;
            use search_os_secrets::{
                derive_legacy_revision_dpapi_entropy, derive_legacy_revision_key_binding,
            };
            use search_os_secrets_windows::{
                LegacyRevisionRootSecretRequirement,
                load_existing_legacy_revision_root_secret_with_context,
                load_or_create_legacy_revision_root_secret_with_context,
            };
            let namespace_id = root.namespace();
            let secret = root.credential_call(create, |namespace, deadline, cancellation| {
                if create {
                    load_or_create_legacy_revision_root_secret_with_context(
                        &namespace,
                        LegacyRevisionRootSecretRequirement::CreateIfMissing,
                        deadline,
                        cancellation,
                    )
                    .map_err(context_error)
                } else {
                    load_existing_legacy_revision_root_secret_with_context(
                        &namespace,
                        deadline,
                        cancellation,
                    )
                    .map_err(context_error)?
                    .ok_or(OwnerError::OwnerRecoveryQuarantined)
                }
            })?;
            let key_binding_digest = derive_legacy_revision_key_binding::<DirectRevisionDigest>(
                &namespace_id,
                secret.expose_secret(),
            );
            let entropy = derive_legacy_revision_dpapi_entropy::<DirectRevisionDigest>(
                &namespace_id,
                secret.expose_secret(),
            );
            drop(secret);
            let protector = Self {
                namespace_id,
                key_binding_digest,
                entropy,
            };
            root.checkpoint("protector-selected")?;
            Ok(protector)
        }
        #[cfg(not(windows))]
        {
            let _ = (root, create);
            Err(OwnerError::OwnerRecoveryQuarantined)
        }
    }

    pub(crate) const fn initialization_key_binding(&self) -> Result<[u8; 32], OwnerError> {
        #[cfg(windows)]
        {
            Ok(self.key_binding_digest)
        }
        #[cfg(not(windows))]
        {
            Err(OwnerError::OwnerRecoveryQuarantined)
        }
    }
}

#[cfg(windows)]
const fn context_error(
    error: search_os_secrets_windows::LegacyRevisionRootSecretContextError,
) -> OwnerError {
    use search_os_secrets_windows::{
        LegacyRevisionRootSecretContextError as Context,
        LegacyRevisionRootSecretError as Credential,
    };
    match error {
        Context::Cancelled => OwnerError::OwnerCancelledBeforeMutation,
        Context::DeadlineExpired => OwnerError::OwnerDeadlineInvalid,
        Context::Credential(Credential::CredentialWriteOutcomeUnknown) => {
            OwnerError::OwnerAcquireOutcomeUnknown
        }
        Context::Credential(_) => OwnerError::OwnerRecoveryQuarantined,
    }
}

impl RevisionProtector {
    pub(crate) fn for_existing_read(
        namespace_id: [u8; 32],
        selected_binding: Option<[u8; 32]>,
        deadline: Instant,
        cancellation: &AtomicBool,
    ) -> Result<Self, OwnerError> {
        #[cfg(windows)]
        {
            use search_os_secrets::{
                derive_legacy_revision_dpapi_entropy, derive_legacy_revision_key_binding,
            };
            use search_os_secrets_windows::load_existing_legacy_revision_root_secret_with_context;

            use super::super::envelope::DirectRevisionDigest;

            let secret = load_existing_legacy_revision_root_secret_with_context(
                &namespace_id,
                deadline,
                cancellation,
            )
            .map_err(context_error)?
            .ok_or(OwnerError::OwnerRecoveryQuarantined)?;
            let key_binding_digest = derive_legacy_revision_key_binding::<DirectRevisionDigest>(
                &namespace_id,
                secret.expose_secret(),
            );
            let entropy = derive_legacy_revision_dpapi_entropy::<DirectRevisionDigest>(
                &namespace_id,
                secret.expose_secret(),
            );
            drop(secret);
            // Install the existing zeroizing Drop before any fallible final check.
            let protector = Self {
                namespace_id,
                key_binding_digest,
                entropy,
            };
            if selected_binding.is_some_and(|expected| expected != key_binding_digest) {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            if cancellation.load(std::sync::atomic::Ordering::Acquire) {
                return Err(OwnerError::OwnerCancelledBeforeMutation);
            }
            if Instant::now() >= deadline {
                return Err(OwnerError::OwnerDeadlineInvalid);
            }
            Ok(protector)
        }
        #[cfg(not(windows))]
        {
            let _ = (namespace_id, selected_binding, deadline, cancellation);
            Err(OwnerError::OwnerRecoveryQuarantined)
        }
    }
}

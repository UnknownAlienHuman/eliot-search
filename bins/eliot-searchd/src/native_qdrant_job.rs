//! Native Windows producer for Qdrant's contained process lifecycle.
//!
//! The supervisor still owns artifact qualification, secret-free argv/config
//! validation, loopback checks, and lifecycle policy. This adapter owns only
//! the concrete suspended spawn, Job Object assignment, held-directory ACL
//! observation, and exact primary-thread resume.

use std::fs::File;

use search_qdrant_supervisor::{ContainedLaunchError, LaunchPlan, SecretMaterial, SpawnedChild};

use crate::native_file::StorageSecurityContext;

#[cfg(windows)]
#[allow(unsafe_code)]
#[path = "native_qdrant_job/windows.rs"]
mod windows;

/// Redacted failure from the native contained-spawn producer.
pub enum NativeQdrantJobError<E> {
    /// The original caller deadline, cancellation, or work allowance refused.
    Context(E),
    /// The held data directory did not satisfy the existing owner-private ACL policy.
    DataRootAclUnqualified,
    /// A bounded Win32 operation or postcondition failed.
    NativeOperationFailed,
    /// The exact primary-thread candidate set was empty, ambiguous, or malformed.
    PrimaryThreadAmbiguous,
    /// The primary thread's prior suspend count was not exactly one.
    PrimaryThreadResumeRefused,
    /// This target has no qualified native Qdrant containment producer.
    UnsupportedPlatform,
}

impl<E> core::fmt::Debug for NativeQdrantJobError<E> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let code = match self {
            Self::Context(_) => "NATIVE_QDRANT_CONTEXT_REFUSED",
            Self::DataRootAclUnqualified => "NATIVE_QDRANT_DATA_ROOT_ACL_UNQUALIFIED",
            Self::NativeOperationFailed => "NATIVE_QDRANT_NATIVE_OPERATION_FAILED",
            Self::PrimaryThreadAmbiguous => "NATIVE_QDRANT_PRIMARY_THREAD_AMBIGUOUS",
            Self::PrimaryThreadResumeRefused => "NATIVE_QDRANT_PRIMARY_THREAD_RESUME_REFUSED",
            Self::UnsupportedPlatform => "NATIVE_QDRANT_UNSUPPORTED_PLATFORM",
        };
        formatter.write_str(code)
    }
}

/// Starts one already-qualified Qdrant plan through the native contained-spawn port.
///
/// `data_directory` must be the exact held directory used for `plan`; the ACL
/// owner is freshly observed through `native_file::observe_owner_private_acl`.
/// Every producer-side native call and bounded enumeration uses `context`.
/// The returned child owns its Job handle through supervisor shutdown/recovery.
pub fn spawn_qualified<C: StorageSecurityContext>(
    plan: &LaunchPlan,
    secret: &SecretMaterial,
    data_directory: &File,
    expected_current_user_digest: [u8; 32],
    context: &mut C,
) -> Result<SpawnedChild, ContainedLaunchError<NativeQdrantJobError<C::Error>>> {
    #[cfg(windows)]
    {
        windows::spawn_qualified(
            plan,
            secret,
            data_directory,
            expected_current_user_digest,
            context,
        )
    }
    #[cfg(not(windows))]
    {
        let _ = (
            plan,
            secret,
            data_directory,
            expected_current_user_digest,
            context,
        );
        Err(ContainedLaunchError::Port(
            NativeQdrantJobError::UnsupportedPlatform,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(windows, test))]
enum ThreadCandidateError {
    ZeroId,
    Multiple,
}

#[cfg(any(windows, test))]
const fn record_thread_candidate(
    selected: &mut Option<u32>,
    candidate: u32,
) -> Result<(), ThreadCandidateError> {
    if candidate == 0 {
        return Err(ThreadCandidateError::ZeroId);
    }
    if selected.is_some() {
        return Err(ThreadCandidateError::Multiple);
    }
    *selected = Some(candidate);
    Ok(())
}

#[cfg(any(windows, test))]
const fn original_suspend_count_is_single(count: u32) -> bool {
    count == 1
}

#[cfg(test)]
mod tests {
    use super::{ThreadCandidateError, original_suspend_count_is_single, record_thread_candidate};

    #[test]
    fn primary_thread_selection_requires_one_nonzero_candidate() {
        let mut selected = None;
        record_thread_candidate(&mut selected, 41).unwrap();
        assert_eq!(selected, Some(41));
        assert_eq!(
            record_thread_candidate(&mut None, 0),
            Err(ThreadCandidateError::ZeroId)
        );
        let mut ambiguous = Some(41);
        assert_eq!(
            record_thread_candidate(&mut ambiguous, 42),
            Err(ThreadCandidateError::Multiple)
        );
    }

    #[test]
    fn resume_is_admitted_only_for_the_original_single_suspend() {
        assert!(original_suspend_count_is_single(1));
        for other in [0, 2, u32::MAX] {
            assert!(!original_suspend_count_is_single(other));
        }
    }
}

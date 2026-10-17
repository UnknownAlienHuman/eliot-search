//! Native Windows producer for Qdrant's contained process lifecycle.
//!
//! The supervisor still owns artifact qualification, secret-free argv/config
//! validation, loopback checks, and lifecycle policy. This adapter owns only
//! the concrete suspended spawn, Job Object assignment, held-directory ACL
//! observation, and exact primary-thread resume.

use std::fs::File;

use search_qdrant_supervisor::{
    ContainedLaunchError, LaunchPlan, SecretLeaseEvidence, SecretMaterial, SpawnedChild,
};

use crate::native_file::{DirectoryIdentity, Observation, StorageSecurityContext};

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
    /// The qualified command directory and held data-directory handles differ.
    DataDirectoryMismatch,
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
            Self::DataDirectoryMismatch => "NATIVE_QDRANT_DATA_DIRECTORY_MISMATCH",
            Self::UnsupportedPlatform => "NATIVE_QDRANT_UNSUPPORTED_PLATFORM",
        };
        formatter.write_str(code)
    }
}

/// Starts one already-qualified Qdrant plan through the native contained-spawn port.
///
/// `data_directory` is the caller's retained directory handle. The Windows
/// producer opens the supervisor's canonical `current_dir` using the qualified
/// no-follow directory boundary, compares the native volume/file/creation
/// identity against this held handle before resume, and retains that exact pin
/// inside Job custody. The ACL owner is observed on that same pin. Every
/// preflight/native call uses `context`'s original deadline, cancellation and
/// cumulative work allowance; bounded probe/connect waits are clipped to its
/// absolute deadline. The returned child owns Job and directory pins through
/// supervisor shutdown/recovery.
///
/// `live_secret_check` must borrow the same actual lease and issuing context
/// used to construct `secret`. It validates the supplied binding metadata
/// against that live lease on every original checkpoint. Historical ticks or
/// copied metadata alone are insufficient. Root keeps the process outcome
/// outside any outer secret callback that could discard it on a late refusal.
pub fn spawn_qualified<C: StorageSecurityContext>(
    plan: &LaunchPlan,
    secret: &SecretMaterial,
    data_directory: &File,
    expected_current_user_digest: [u8; 32],
    context: &mut C,
    live_secret_check: &mut impl FnMut(&SecretLeaseEvidence) -> Result<(), C::Error>,
) -> Result<SpawnedChild, ContainedLaunchError<NativeQdrantJobError<C::Error>>> {
    #[cfg(windows)]
    {
        let mut guarded = LeaseCheckedContext {
            native: context,
            live_secret_check,
            evidence: plan.secret_lease(),
        };
        windows::spawn_qualified(
            plan,
            secret,
            data_directory,
            expected_current_user_digest,
            &mut guarded,
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
            live_secret_check,
        );
        Err(ContainedLaunchError::Port(
            NativeQdrantJobError::UnsupportedPlatform,
        ))
    }
}

#[cfg(windows)]
struct LeaseCheckedContext<'a, C, F> {
    native: &'a mut C,
    live_secret_check: &'a mut F,
    evidence: &'a SecretLeaseEvidence,
}

#[cfg(windows)]
impl<C, F> StorageSecurityContext for LeaseCheckedContext<'_, C, F>
where
    C: StorageSecurityContext,
    F: FnMut(&SecretLeaseEvidence) -> Result<(), C::Error>,
{
    type Error = C::Error;

    fn absolute_deadline(&self) -> std::time::Instant {
        self.native.absolute_deadline()
    }

    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.native.checkpoint()?;
        (self.live_secret_check)(self.evidence)?;
        self.native.checkpoint()
    }

    fn charge(&mut self, work: crate::native_file::StorageSecurityWork) -> Result<(), Self::Error> {
        self.native.charge(work)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectoryBindingError {
    NotDirectory,
    Mismatch,
}

const fn identity_from_observation(
    observation: Observation,
) -> Result<DirectoryIdentity, DirectoryBindingError> {
    if observation.attributes & 0x10 == 0 {
        return Err(DirectoryBindingError::NotDirectory);
    }
    Ok(DirectoryIdentity {
        volume_serial: observation.volume_serial,
        file_index: observation.file_index,
        creation_time: observation.creation_time,
        attributes: observation.attributes,
    })
}

#[cfg(test)]
fn compare_directory_observation(
    expected: DirectoryIdentity,
    observed: Observation,
) -> Result<(), DirectoryBindingError> {
    let actual = identity_from_observation(observed)?;
    compare_directory_identities(expected, actual)
}

fn compare_directory_identities(
    expected: DirectoryIdentity,
    observed: DirectoryIdentity,
) -> Result<(), DirectoryBindingError> {
    if observed == expected {
        Ok(())
    } else {
        Err(DirectoryBindingError::Mismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DirectoryBindingError, DirectoryIdentity, Observation, ThreadCandidateError,
        compare_directory_observation, original_suspend_count_is_single, record_thread_candidate,
    };

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

    #[test]
    fn held_data_directory_must_match_the_qualified_command_directory_identity() {
        let expected = DirectoryIdentity {
            volume_serial: 7,
            file_index: 11,
            creation_time: 13,
            attributes: 0x10,
        };
        let observed = Observation {
            volume_serial: 7,
            file_index: 11,
            creation_time: 13,
            attributes: 0x10,
            length: 0,
            last_write_time: 17,
        };
        assert_eq!(compare_directory_observation(expected, observed), Ok(()));
        assert_eq!(
            compare_directory_observation(
                expected,
                Observation {
                    file_index: 12,
                    ..observed
                },
            ),
            Err(DirectoryBindingError::Mismatch)
        );
        assert_eq!(
            compare_directory_observation(
                expected,
                Observation {
                    attributes: 0x80,
                    ..observed
                },
            ),
            Err(DirectoryBindingError::NotDirectory)
        );
    }
}

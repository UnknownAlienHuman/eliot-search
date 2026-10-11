use super::context::{OperationContext, RootSecretCheckpoint};
use super::*;
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};
use std::rc::Rc;
use std::time::Instant;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Seam {
    Read(usize),
    Generate,
    Lock(usize),
    Sleep(usize),
    Write,
    Release,
}

struct FakeVaultGuard<'a> {
    active: Rc<Cell<usize>>,
    cancellation: Option<&'a AtomicBool>,
    cancel_on_release: bool,
}

impl Drop for FakeVaultGuard<'_> {
    fn drop(&mut self) {
        self.active.set(self.active.get() - 1);
        if self.cancel_on_release
            && let Some(flag) = self.cancellation
        {
            flag.store(true, Ordering::Release);
        }
    }
}

#[derive(Default)]
struct FakePlatform<'a> {
    credential: Option<[u8; LEGACY_REVISION_ROOT_SECRET_BYTES]>,
    generated: [u8; LEGACY_REVISION_ROOT_SECRET_BYTES],
    inject_on_first_lock: Option<[u8; LEGACY_REVISION_ROOT_SECRET_BYTES]>,
    lock_attempts: usize,
    reads: usize,
    writes: usize,
    generated_count: usize,
    sleeps: Vec<Duration>,
    read_error: Option<LegacyRevisionRootSecretError>,
    write_error: Option<LegacyRevisionRootSecretError>,
    corrupt_readback: bool,
    targets: Vec<Vec<u16>>,
    active_guards: Rc<Cell<usize>>,
    writes_under_guard: bool,
    lock_failures_remaining: usize,
    locked_read_failures_remaining: usize,
    readback_error: Option<LegacyRevisionRootSecretError>,
    absent_readback: bool,
    persist_then_error: bool,
    cancellation: Option<&'a AtomicBool>,
    cancel_at: Option<Seam>,
}

impl FakePlatform<'_> {
    fn observe_seam(&self, seam: Seam) {
        if self.cancel_at == Some(seam)
            && let Some(flag) = self.cancellation
        {
            flag.store(true, Ordering::Release);
        }
    }
}

impl<'a> RootSecretPlatform for FakePlatform<'a> {
    type VaultGuard = FakeVaultGuard<'a>;

    fn read_credential(
        &mut self,
        target: &[u16],
    ) -> Result<Option<LegacyRevisionRootSecret>, LegacyRevisionRootSecretError> {
        self.reads += 1;
        self.targets.push(target.to_vec());
        self.observe_seam(Seam::Read(self.reads));
        if self.writes > 0 {
            assert_eq!(self.active_guards.get(), 1, "readback retains vault guard");
            if let Some(error) = self.readback_error.take() {
                return Err(error);
            }
            if self.absent_readback {
                return Ok(None);
            }
        } else if self.active_guards.get() > 0 && self.locked_read_failures_remaining > 0 {
            self.locked_read_failures_remaining -= 1;
            return Err(LegacyRevisionRootSecretError::CredentialReadFailed(5));
        }
        if let Some(error) = self.read_error.take() {
            return Err(error);
        }
        let value = self.credential.map(|mut bytes| {
            if self.corrupt_readback && self.writes > 0 {
                bytes[0] ^= 0xff;
            }
            LegacyRevisionRootSecret::from_bytes(bytes)
        });
        Ok(value)
    }

    fn write_credential(
        &mut self,
        target: &[u16],
        secret: &mut LegacyRevisionRootSecret,
    ) -> Result<(), LegacyRevisionRootSecretError> {
        self.writes += 1;
        self.targets.push(target.to_vec());
        self.writes_under_guard = self.active_guards.get() == 1;
        self.observe_seam(Seam::Write);
        if self.persist_then_error {
            self.credential = Some(*secret.expose_secret());
        }
        if let Some(error) = self.write_error.take() {
            return Err(error);
        }
        self.credential = Some(*secret.expose_secret());
        Ok(())
    }

    fn generate_root_secret(
        &mut self,
    ) -> Result<LegacyRevisionRootSecret, LegacyRevisionRootSecretError> {
        self.generated_count += 1;
        self.observe_seam(Seam::Generate);
        Ok(LegacyRevisionRootSecret::from_bytes(self.generated))
    }

    fn acquire_vault_lock(&mut self) -> Result<Self::VaultGuard, ()> {
        self.lock_attempts += 1;
        self.observe_seam(Seam::Lock(self.lock_attempts));
        if self.lock_failures_remaining > 0 {
            self.lock_failures_remaining -= 1;
            return Err(());
        }
        if self.lock_attempts == 1
            && let Some(secret) = self.inject_on_first_lock.take()
        {
            self.credential = Some(secret);
        }
        self.active_guards.set(self.active_guards.get() + 1);
        Ok(FakeVaultGuard {
            active: Rc::clone(&self.active_guards),
            cancellation: self.cancellation,
            cancel_on_release: self.cancel_at == Some(Seam::Release),
        })
    }

    fn sleep(&mut self, duration: Duration) {
        self.sleeps.push(duration);
        self.observe_seam(Seam::Sleep(self.sleeps.len()));
    }
}

#[test]
fn credential_target_is_exact_lower_hex_and_terminated() {
    let target = credential_target(&[0xab; 32]);
    let text = String::from_utf16(&target[..target.len() - 1]).expect("target");
    assert_eq!(
        text,
        "ELIOT Search/revision-key/abababababababababababababababababababababababababababababababab"
    );
    assert_eq!(target.last(), Some(&0));
}

#[test]
fn existing_secret_is_returned_without_rng_or_write() {
    let mut platform = FakePlatform {
        credential: Some([0x11; LEGACY_REVISION_ROOT_SECRET_BYTES]),
        generated: [0x22; LEGACY_REVISION_ROOT_SECRET_BYTES],
        ..FakePlatform::default()
    };
    let secret = load_or_create_with_platform(
        &mut platform,
        &[0x33; 32],
        LegacyRevisionRootSecretRequirement::CreateIfMissing,
    )
    .expect("existing secret");
    assert_eq!(secret.expose_secret(), &[0x11; 32]);
    assert_eq!(platform.generated_count, 0);
    assert_eq!(platform.writes, 0);
    assert_eq!(platform.lock_attempts, 0);
}

#[test]
fn create_path_rechecks_after_lock_and_never_overwrites_a_racing_key() {
    let mut platform = FakePlatform {
        generated: [0x44; LEGACY_REVISION_ROOT_SECRET_BYTES],
        inject_on_first_lock: Some([0x55; LEGACY_REVISION_ROOT_SECRET_BYTES]),
        ..FakePlatform::default()
    };
    let secret = load_or_create_with_platform(
        &mut platform,
        &[0x66; 32],
        LegacyRevisionRootSecretRequirement::CreateIfMissing,
    )
    .expect("racing key");
    assert_eq!(secret.expose_secret(), &[0x55; 32]);
    assert_eq!(platform.generated_count, 1);
    assert_eq!(platform.writes, 0);
    assert_eq!(platform.lock_attempts, 1);
}

#[test]
fn create_path_writes_and_requires_exact_readback() {
    let mut platform = FakePlatform {
        generated: [0x77; LEGACY_REVISION_ROOT_SECRET_BYTES],
        ..FakePlatform::default()
    };
    let secret = load_or_create_with_platform(
        &mut platform,
        &[0x88; 32],
        LegacyRevisionRootSecretRequirement::CreateIfMissing,
    )
    .expect("created secret");
    assert_eq!(secret.expose_secret(), &[0x77; 32]);
    assert_eq!(platform.writes, 1);

    let mut corrupt = FakePlatform {
        generated: [0x99; LEGACY_REVISION_ROOT_SECRET_BYTES],
        corrupt_readback: true,
        ..FakePlatform::default()
    };
    assert!(matches!(
        load_or_create_with_platform(
            &mut corrupt,
            &[0xaa; 32],
            LegacyRevisionRootSecretRequirement::CreateIfMissing,
        ),
        Err(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown)
    ));
    assert_eq!(corrupt.writes, 1);
}

#[test]
fn protected_objects_never_authorize_replacement_key_creation() {
    let mut platform = FakePlatform::default();
    assert!(matches!(
        load_or_create_with_platform(
            &mut platform,
            &[0xbb; 32],
            LegacyRevisionRootSecretRequirement::RequireExisting,
        ),
        Err(LegacyRevisionRootSecretError::MissingExistingCredential)
    ));
    assert_eq!(platform.generated_count, 0);
    assert_eq!(platform.writes, 0);
    assert_eq!(platform.lock_attempts, ROOT_SECRET_ATTEMPTS as usize);
    assert_eq!(platform.sleeps.len(), ROOT_SECRET_ATTEMPTS as usize);
}

#[test]
fn secret_debug_and_error_reasons_are_redacted_and_stable() {
    let secret = LegacyRevisionRootSecret::from_bytes([0xcd; 32]);
    let debug = format!("{secret:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("205"));
    assert_eq!(
        LegacyRevisionRootSecretError::CredentialReadFailed(5).to_string(),
        "WINDOWS_REVISION_KEY_READ_FAILED:5"
    );
    assert_eq!(
        LegacyRevisionRootSecretError::RandomGenerationFailed(-7).to_string(),
        "WINDOWS_REVISION_RNG_FAILED:-7"
    );
}

#[test]
fn every_unverified_publication_is_terminal_after_one_write() {
    for case in 0..7 {
        let mut platform = FakePlatform {
            generated: [0x71; 32],
            ..FakePlatform::default()
        };
        match case {
            0 => {
                platform.write_error = Some(LegacyRevisionRootSecretError::CredentialWriteFailed(5))
            }
            1 => {
                platform.persist_then_error = true;
                platform.write_error =
                    Some(LegacyRevisionRootSecretError::CredentialWriteFailed(5));
            }
            2 => platform.absent_readback = true,
            3 => {
                platform.readback_error =
                    Some(LegacyRevisionRootSecretError::CredentialReadFailed(5))
            }
            4 => {
                platform.readback_error =
                    Some(LegacyRevisionRootSecretError::CredentialReadbackInvalid)
            }
            5 => platform.corrupt_readback = true,
            6 => platform.write_error = Some(LegacyRevisionRootSecretError::CredentialTooLarge),
            _ => unreachable!(),
        }
        let result = load_or_create_with_platform(
            &mut platform,
            &[0x81; 32],
            LegacyRevisionRootSecretRequirement::CreateIfMissing,
        );
        assert!(matches!(
            result,
            Err(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown)
        ));
        assert_eq!(platform.writes, 1);
        assert_eq!(platform.generated_count, 1);
        assert_eq!(platform.lock_attempts, 1);
        assert!(platform.sleeps.is_empty());
        assert!(platform.writes_under_guard);
        assert_eq!(platform.active_guards.get(), 0);
        assert!(
            platform
                .targets
                .iter()
                .all(|target| *target == credential_target(&[0x81; 32]))
        );
        if case == 1 {
            assert_eq!(platform.credential, Some([0x71; 32]));
        }
    }
}

#[test]
fn acquisition_retries_remain_finite_and_precede_the_only_publication() {
    for (locks, reads, expected_writes) in [(7, 0, 1), (8, 0, 0), (0, 2, 1)] {
        let mut platform = FakePlatform {
            generated: [0x19; 32],
            lock_failures_remaining: locks,
            locked_read_failures_remaining: reads,
            ..FakePlatform::default()
        };
        let result = load_or_create_with_platform(
            &mut platform,
            &[0x29; 32],
            LegacyRevisionRootSecretRequirement::CreateIfMissing,
        );
        assert_eq!(platform.writes, expected_writes);
        assert_eq!(platform.generated_count, 1);
        assert!(platform.lock_attempts <= ROOT_SECRET_ATTEMPTS as usize);
        assert_eq!(platform.active_guards.get(), 0);
        if expected_writes == 1 {
            assert_eq!(result.expect("verified key").expose_secret(), &[0x19; 32]);
            assert!(platform.writes_under_guard);
        } else {
            assert!(matches!(
                result,
                Err(LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown)
            ));
        }
    }
}

#[test]
fn original_cancellation_before_publication_stops_every_later_seam() {
    for seam in [
        Seam::Read(1),
        Seam::Generate,
        Seam::Lock(1),
        Seam::Read(2),
        Seam::Sleep(1),
    ] {
        let cancellation = AtomicBool::new(false);
        let context =
            OperationContext::new(Instant::now() + Duration::from_secs(60), &cancellation);
        let mut platform = FakePlatform {
            generated: [0x31; 32],
            cancellation: Some(&cancellation),
            cancel_at: Some(seam),
            lock_failures_remaining: usize::from(seam == Seam::Sleep(1)),
            ..FakePlatform::default()
        };
        let result = load_or_create_checked(
            &mut platform,
            &[0x41; 32],
            LegacyRevisionRootSecretRequirement::CreateIfMissing,
            &context,
        );
        assert!(matches!(
            result,
            Err(LegacyRevisionRootSecretContextError::Cancelled)
        ));
        assert!(cancellation.load(Ordering::Acquire));
        assert_eq!(platform.writes, 0);
        assert_eq!(platform.active_guards.get(), 0);
        assert_eq!(
            platform.lock_attempts,
            usize::from(!matches!(seam, Seam::Read(1) | Seam::Generate))
        );
    }
}

#[test]
fn post_write_cancellation_is_unknown_even_with_matching_readback() {
    for seam in [Seam::Write, Seam::Read(3)] {
        let cancellation = AtomicBool::new(false);
        let context =
            OperationContext::new(Instant::now() + Duration::from_secs(60), &cancellation);
        let mut platform = FakePlatform {
            generated: [0x51; 32],
            cancellation: Some(&cancellation),
            cancel_at: Some(seam),
            ..FakePlatform::default()
        };
        let result = load_or_create_checked(
            &mut platform,
            &[0x61; 32],
            LegacyRevisionRootSecretRequirement::CreateIfMissing,
            &context,
        );
        assert!(matches!(
            result,
            Err(LegacyRevisionRootSecretContextError::Credential(
                LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown
            ))
        ));
        assert_eq!(platform.credential, Some([0x51; 32]));
        assert_eq!(platform.writes, 1);
        assert_eq!(platform.generated_count, 1);
        assert!(platform.writes_under_guard);
        assert_eq!(platform.active_guards.get(), 0);
    }
}

#[test]
fn cancellation_during_vault_release_prevents_success() {
    for (requirement, racing_key) in [
        (LegacyRevisionRootSecretRequirement::CreateIfMissing, false),
        (LegacyRevisionRootSecretRequirement::CreateIfMissing, true),
        (LegacyRevisionRootSecretRequirement::RequireExisting, true),
    ] {
        let cancellation = AtomicBool::new(false);
        let context =
            OperationContext::new(Instant::now() + Duration::from_secs(60), &cancellation);
        let mut platform = FakePlatform {
            generated: [0x51; 32],
            inject_on_first_lock: racing_key.then_some([0x41; 32]),
            cancellation: Some(&cancellation),
            cancel_at: Some(Seam::Release),
            ..FakePlatform::default()
        };
        let result = load_or_create_checked(&mut platform, &[0x61; 32], requirement, &context);
        assert_eq!(platform.active_guards.get(), 0);
        assert!(cancellation.load(Ordering::Acquire));
        assert_eq!(platform.writes, usize::from(!racing_key));
        if racing_key {
            assert!(matches!(
                result,
                Err(LegacyRevisionRootSecretContextError::Cancelled)
            ));
            assert_eq!(platform.credential, Some([0x41; 32]));
        } else {
            assert!(matches!(
                result,
                Err(LegacyRevisionRootSecretContextError::Credential(
                    LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown
                ))
            ));
            assert_eq!(platform.credential, Some([0x51; 32]));
        }
    }
}

#[test]
fn existing_only_context_read_never_generates_or_publishes() {
    for credential in [None, Some([0x11; 32])] {
        let cancellation = AtomicBool::new(false);
        let context =
            OperationContext::new(Instant::now() + Duration::from_secs(60), &cancellation);
        let mut platform = FakePlatform {
            credential,
            ..FakePlatform::default()
        };
        let result = read_with_platform(&mut platform, &credential_target(&[0x22; 32]), &context)
            .expect("existing-only read");
        assert_eq!(
            result.as_ref().map(|secret| *secret.expose_secret()),
            credential
        );
        assert_eq!(platform.reads, 1);
        assert_eq!(platform.generated_count, 0);
        assert_eq!(platform.writes, 0);
        assert_eq!(platform.lock_attempts, 0);
    }
    let cancellation = AtomicBool::new(false);
    let context = OperationContext::new(Instant::now() + Duration::from_secs(60), &cancellation);
    let mut platform = FakePlatform {
        credential: Some([0x33; 32]),
        cancellation: Some(&cancellation),
        cancel_at: Some(Seam::Read(1)),
        ..FakePlatform::default()
    };
    assert!(matches!(
        read_with_platform(&mut platform, &credential_target(&[0x44; 32]), &context),
        Err(LegacyRevisionRootSecretContextError::Cancelled)
    ));
    assert_eq!(platform.writes, 0);
    assert_eq!(platform.generated_count, 0);
}

struct ExpiryCheckpoint {
    checks: Cell<usize>,
    expires_at: usize,
}

impl RootSecretCheckpoint for ExpiryCheckpoint {
    type Error = LegacyRevisionRootSecretContextError;

    fn check(&self) -> Result<(), Self::Error> {
        self.checks.set(self.checks.get() + 1);
        if self.checks.get() >= self.expires_at {
            Err(Self::Error::DeadlineExpired)
        } else {
            Ok(())
        }
    }

    fn limit_sleep(&self, duration: Duration) -> Duration {
        duration
    }
}

#[test]
fn terminal_checkpoint_refusals_preserve_the_possible_write_boundary() {
    // Deterministic kernel-phase expiry injection; not native clock qualification.
    for expires_at in 1..=15 {
        let context = ExpiryCheckpoint {
            checks: Cell::new(0),
            expires_at,
        };
        let mut platform = FakePlatform {
            generated: [0x91; 32],
            ..FakePlatform::default()
        };
        let result = load_or_create_checked(
            &mut platform,
            &[0xa1; 32],
            LegacyRevisionRootSecretRequirement::CreateIfMissing,
            &context,
        );
        if expires_at <= 10 {
            assert!(matches!(
                result,
                Err(LegacyRevisionRootSecretContextError::DeadlineExpired)
            ));
            assert_eq!(platform.writes, 0);
        } else {
            assert!(matches!(
                result,
                Err(LegacyRevisionRootSecretContextError::Credential(
                    LegacyRevisionRootSecretError::CredentialWriteOutcomeUnknown
                ))
            ));
            assert_eq!(platform.writes, 1);
        }
        assert_eq!(platform.active_guards.get(), 0);
    }
}

#[test]
fn elapsed_original_deadline_refuses_before_any_platform_call() {
    let cancellation = AtomicBool::new(false);
    let deadline = Instant::now();
    let context = OperationContext::new(deadline, &cancellation);
    let mut platform = FakePlatform::default();
    let result = load_or_create_checked(
        &mut platform,
        &[0xb1; 32],
        LegacyRevisionRootSecretRequirement::CreateIfMissing,
        &context,
    );
    assert!(matches!(
        result,
        Err(LegacyRevisionRootSecretContextError::DeadlineExpired)
    ));
    assert_eq!(
        (
            platform.reads,
            platform.generated_count,
            platform.lock_attempts,
            platform.writes
        ),
        (0, 0, 0, 0)
    );
    let deadline = Instant::now() + Duration::from_millis(50);
    let bounded = OperationContext::new(deadline, &cancellation);
    let delay = bounded.limit_sleep(Duration::from_secs(1));
    assert!(delay <= Duration::from_millis(50));
    assert_eq!(bounded.limit_sleep(Duration::ZERO), Duration::ZERO);
}

#[test]
fn contextual_errors_expose_only_stable_codes() {
    let error = LegacyRevisionRootSecretContextError::Credential(
        LegacyRevisionRootSecretError::CredentialReadFailed(123_456_789),
    );
    assert_eq!(format!("{error:?}"), "WINDOWS_REVISION_KEY_READ_FAILED");
    assert_eq!(error.to_string(), "WINDOWS_REVISION_KEY_READ_FAILED");
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(
        LegacyRevisionRootSecretContextError::Cancelled.code(),
        "WINDOWS_REVISION_KEY_CANCELLED"
    );
    assert_eq!(
        LegacyRevisionRootSecretContextError::DeadlineExpired.code(),
        "WINDOWS_REVISION_KEY_DEADLINE_EXPIRED"
    );
}

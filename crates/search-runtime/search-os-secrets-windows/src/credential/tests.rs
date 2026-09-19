use super::*;

#[derive(Default)]
struct FakePlatform {
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
}

impl RootSecretPlatform for FakePlatform {
    type VaultGuard = ();

    fn read_credential(
        &mut self,
        _target: &[u16],
    ) -> Result<Option<LegacyRevisionRootSecret>, LegacyRevisionRootSecretError> {
        self.reads += 1;
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
        _target: &[u16],
        secret: &mut LegacyRevisionRootSecret,
    ) -> Result<(), LegacyRevisionRootSecretError> {
        self.writes += 1;
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
        Ok(LegacyRevisionRootSecret::from_bytes(self.generated))
    }

    fn acquire_vault_lock(&mut self) -> Result<Self::VaultGuard, ()> {
        self.lock_attempts += 1;
        if self.lock_attempts == 1
            && let Some(secret) = self.inject_on_first_lock.take()
        {
            self.credential = Some(secret);
        }
        Ok(())
    }

    fn sleep(&mut self, duration: Duration) {
        self.sleeps.push(duration);
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
        Err(LegacyRevisionRootSecretError::CredentialReadbackMismatch)
    ));
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

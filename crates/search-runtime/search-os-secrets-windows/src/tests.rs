use super::*;

#[test]
fn scopes_are_canonical_and_secret_debug_is_redacted() {
    let scope = ProtectionScope::new(
        "user",
        "installation",
        "incarnation",
        "purpose",
    )
    .expect("scope");
    assert!(!scope.as_entropy().is_empty());
    assert_eq!(
        ProtectionScope::new(" user", "installation", "incarnation", "purpose"),
        Err(DpapiError::InvalidScope)
    );

    let secret = SecretBytes::new(b"private".to_vec()).expect("secret");
    let debug = format!("{secret:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("private"));
}

#[test]
fn legacy_revision_failure_codes_are_stable() {
    assert_eq!(MAX_LEGACY_REVISION_DPAPI_BYTES, 65 * 1024 * 1024);
    assert_eq!(LEGACY_REVISION_DPAPI_ENTROPY_BYTES, 32);
    assert_eq!(
        LegacyRevisionDpapiError::InputTooLarge.code(),
        "DPAPI_LEGACY_REVISION_INPUT_TOO_LARGE"
    );
    assert_eq!(
        LegacyRevisionDpapiError::ProtectFailed(5).to_string(),
        "DPAPI_LEGACY_REVISION_PROTECT_FAILED:5"
    );
    assert_eq!(
        LegacyRevisionDpapiError::UnprotectFailed(13).to_string(),
        "DPAPI_LEGACY_REVISION_UNPROTECT_FAILED:13"
    );
}

#[cfg(not(windows))]
#[test]
fn non_windows_operations_fail_closed() {
    let scope = ProtectionScope::new("user", "install", "incarnation", "purpose")
        .expect("scope");
    let secret = SecretBytes::new(b"secret".to_vec()).expect("secret");
    assert_eq!(
        protect_current_user(&secret, &scope),
        Err(DpapiError::UnsupportedPlatform)
    );

    let mut input = b"legacy-inner".to_vec();
    assert_eq!(
        protect_legacy_revision_current_user(&mut input, &[0x44; 32]),
        Err(LegacyRevisionDpapiError::UnsupportedPlatform)
    );
    assert!(matches!(
        load_existing_legacy_revision_root_secret(&[0x55; 32]),
        Err(LegacyRevisionRootSecretError::UnsupportedPlatform)
    ));
    assert!(matches!(
        load_or_create_legacy_revision_root_secret(
            &[0x55; 32],
            LegacyRevisionRootSecretRequirement::CreateIfMissing,
        ),
        Err(LegacyRevisionRootSecretError::UnsupportedPlatform)
    ));
}

#[cfg(windows)]
#[test]
fn current_user_short_secret_round_trip_is_scope_bound() {
    let scope = ProtectionScope::new(
        "cargo-test-current-user",
        "eliot-search-ci",
        "dpapi-regression-v1",
        "revision-store-master-key",
    )
    .expect("scope");
    let wrong_scope = ProtectionScope::new(
        "cargo-test-current-user",
        "eliot-search-ci",
        "dpapi-regression-v1",
        "wrong-purpose",
    )
    .expect("wrong scope");
    let plaintext =
        SecretBytes::new(b"bounded-dpapi-round-trip".to_vec()).expect("plaintext");

    let protected = protect_current_user(&plaintext, &scope).expect("protect");
    assert_ne!(protected.as_bytes(), plaintext.expose_secret());
    let recovered = unprotect_current_user(&protected, &scope).expect("unprotect");
    assert_eq!(recovered.expose_secret(), plaintext.expose_secret());
    assert!(unprotect_current_user(&protected, &wrong_scope).is_err());
}

#[cfg(windows)]
#[test]
fn current_user_legacy_revision_round_trip_is_entropy_bound() {
    let original = b"legacy-revision-inner-envelope".to_vec();
    let mut input = original.clone();
    let entropy = [0x44; 32];
    let wrong_entropy = [0x45; 32];

    let mut protected =
        protect_legacy_revision_current_user(&mut input, &entropy).expect("protect");
    assert_ne!(protected, original);
    let mut wrong = protected.clone();
    assert!(
        unprotect_legacy_revision_current_user(&mut wrong, &wrong_entropy).is_err()
    );
    let recovered =
        unprotect_legacy_revision_current_user(&mut protected, &entropy)
            .expect("unprotect");
    assert_eq!(recovered, original);
}

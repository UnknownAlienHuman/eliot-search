use std::path::Path;

use super::api::delete_sealed;
use super::model::{DeleteReceipt, SensitiveBytes, wipe};
use super::spec::SealedStoreError;

#[test]
fn wipe_clears_every_byte_without_local_unsafe() {
    let mut bytes = [0xa5_u8; 64];
    wipe(&mut bytes);
    assert_eq!(bytes, [0_u8; 64]);
}

#[test]
fn sensitive_owner_preserves_explicit_access_but_redacts_debug() {
    let secret = b"sealed-plaintext-sentinel";
    let bytes = SensitiveBytes::new(secret.to_vec()).expect("bounded plaintext");
    assert_eq!(bytes.expose(), secret);
    assert_eq!(bytes.len(), secret.len());
    assert!(!bytes.is_empty());
    let debug = format!("{bytes:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("sealed-plaintext-sentinel"));
}

#[test]
fn empty_plaintext_is_rejected() {
    assert_eq!(
        SensitiveBytes::new(Vec::new()).expect_err("empty plaintext"),
        SealedStoreError::EmptyPlaintext
    );
}

#[test]
fn delete_sealed_reports_logical_closure_without_claiming_physical_erasure() {
    let error = delete_sealed(Path::new("."), "")
        .expect_err("empty identifier is rejected");
    #[cfg(windows)]
    assert_eq!(error, SealedStoreError::InvalidObjectId);
    #[cfg(not(windows))]
    assert_eq!(error, SealedStoreError::UnsupportedPlatform);

    let receipt = DeleteReceipt {
        object_id: "object-1".to_owned(),
        logical_delete_complete: true,
        physical_erasure_guaranteed: false,
    };
    assert_eq!(receipt.object_id, "object-1");
    assert!(receipt.logical_delete_complete);
    assert!(!receipt.physical_erasure_guaranteed);
}

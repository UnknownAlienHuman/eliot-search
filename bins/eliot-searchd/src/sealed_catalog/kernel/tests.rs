use super::*;

use std::path::Path;

use crate::sealed_digest::Sha256Digest;
use crate::sealed_transaction::{PutDisposition, SealedTransactionReceipt};

const FIXTURE_DIGEST_HEX: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn fixture_binding() -> SealedCatalogBinding {
    SealedCatalogBinding {
        source_id: "source-1".to_owned(),
        source_revision_id: "revision-1".to_owned(),
        content_operation_id: "content-operation-1".to_owned(),
        content_object_id: "content-object-1".to_owned(),
        content_sha256: Sha256Digest::from_hex(FIXTURE_DIGEST_HEX)
            .expect("fixture digest parses"),
        content_plaintext_bytes: 12,
        content_ciphertext_bytes: 256,
        catalog_format_version: 1,
    }
}

#[test]
fn binding_encode_decode_round_trip_preserves_every_immutable_field() {
    let binding = fixture_binding();
    let manifest = binding.encode().expect("valid binding encodes");
    assert!(manifest.starts_with("ELIOT-SEALED-CATALOG-V1\n"));
    assert!(manifest.contains("source_id=source-1\n"));
    assert!(manifest.contains("content_plaintext_bytes=12\n"));
    let restored =
        SealedCatalogBinding::decode(manifest.as_bytes()).expect("canonical bytes decode");
    assert_eq!(restored, binding);
}

#[test]
fn bind_revision_rejects_a_malformed_identifier_before_touching_storage() {
    let failure = bind_revision(
        Path::new("."),
        "",
        "content-object-1",
        "catalog-operation-1",
        "catalog-object-1",
        "source-1",
        "revision-1",
    )
    .expect_err("empty operation identifier is rejected");
    assert_eq!(failure, SealedCatalogError::InvalidIdentifier);
    assert_eq!(
        SealedCatalogError::CatalogReadbackMismatch.code(),
        "SEALED_CATALOG_READBACK_MISMATCH"
    );
}

#[test]
fn catalog_receipt_carries_the_exact_terminal_binding() {
    let digest = Sha256Digest::from_hex(FIXTURE_DIGEST_HEX).expect("fixture digest parses");
    let binding = fixture_binding();
    let receipt = SealedCatalogReceipt {
        catalog_object_id: "catalog-object-1".to_owned(),
        binding: binding.clone(),
        content_transaction: SealedTransactionReceipt {
            operation_id: "content-operation-1".to_owned(),
            object_id: "content-object-1".to_owned(),
            plaintext_bytes: 12,
            plaintext_sha256: digest,
            ciphertext_bytes: 256,
            disposition: PutDisposition::Replay,
            sealed_readback_verified: true,
            receipt_readback_verified: true,
        },
        catalog_transaction: SealedTransactionReceipt {
            operation_id: "catalog-operation-1".to_owned(),
            object_id: "catalog-object-1".to_owned(),
            plaintext_bytes: 64,
            plaintext_sha256: digest,
            ciphertext_bytes: 512,
            disposition: PutDisposition::Created,
            sealed_readback_verified: true,
            receipt_readback_verified: true,
        },
        catalog_readback_verified: true,
    };
    assert_eq!(receipt.binding, binding);
    assert_eq!(
        receipt.content_transaction.disposition,
        PutDisposition::Replay
    );
    assert_eq!(
        receipt.catalog_transaction.disposition,
        PutDisposition::Created
    );
    assert!(receipt.catalog_readback_verified);
}

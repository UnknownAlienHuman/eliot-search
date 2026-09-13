use super::*;
use crate::MaterializationError;
use crate::profile::{
    SourceEncoding, ValidatedMaterializerProfile, baseline_profile_descriptor,
};
use crate::request::{
    AcceptedProfiles, CancellationToken, DEFAULT_MATERIALIZATION_BUDGET,
    MaterializationRequest, ValidatedMaterializationRequest,
    validate_materialization_request,
};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

fn profile() -> ValidatedMaterializerProfile {
    crate::profile::validate_materializer_profile(&baseline_profile_descriptor(
        "product-test",
        1,
    ))
    .expect("profile")
}

struct FakePort {
    bytes: Vec<u8>,
    digest: Blake3Digest32,
    residency: OpaqueId,
}

impl RevisionReadPort for FakePort {
    fn read_exact(
        &self,
        _source: &OpaqueId,
        _revision: NonZeroRevision,
        _byte_count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError> {
        Ok(StoredRevisionBytes::new(
            self.bytes.clone(),
            self.digest,
            self.residency.clone(),
        ))
    }
}

struct FailingPort;

impl RevisionReadPort for FailingPort {
    fn read_exact(
        &self,
        _source: &OpaqueId,
        _revision: NonZeroRevision,
        _byte_count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError> {
        Err(MaterializationError::RevisionUnavailable)
    }
}

fn digest_for(bytes: &[u8]) -> Blake3Digest32 {
    let mut out = [0_u8; 32];
    for (index, byte) in bytes.iter().enumerate() {
        out[index % 32] ^= byte.wrapping_add(index.to_le_bytes()[0]);
    }
    Blake3Digest32::from_bytes(out)
}

fn validated(
    bytes: &[u8],
    profile: &ValidatedMaterializerProfile,
) -> ValidatedMaterializationRequest {
    let request = MaterializationRequest {
        source_id: OpaqueId::new("source:test").expect("source"),
        revision: NonZeroRevision::new(1).expect("revision"),
        residency: OpaqueId::new("residency:test").expect("residency"),
        content_digest: digest_for(bytes),
        byte_count: u64::try_from(bytes.len()).expect("len"),
        declared_kind: crate::profile::SourceKind::Text,
        declared_encoding: SourceEncoding::Utf8,
        profile_id: profile.id(),
        operation_id: OpaqueId::new("operation:test").expect("operation"),
        from_unsaved_bytes: false,
        unsaved_snapshot_receipt: None,
    };
    validate_materialization_request(
        &request,
        &AcceptedProfiles::new(vec![profile.clone()]),
        &DEFAULT_MATERIALIZATION_BUDGET,
    )
    .expect("request")
}

fn materialize(bytes: &[u8]) -> MaterializationProduct {
    let profile = profile();
    let request = validated(bytes, &profile);
    let port = FakePort {
        bytes: bytes.to_vec(),
        digest: request.content_digest(),
        residency: request.residency().clone(),
    };
    let context = MaterializationContext {
        profile,
        budget: DEFAULT_MATERIALIZATION_BUDGET,
        cancel: CancellationToken::never(),
    };
    materialize_text_or_code(&request, &port, &context).expect("materialize")
}

#[test]
fn revision_open_binds_digest_residency_length() {
    let profile = profile();
    let bytes = b"exact\n".as_slice();
    let request = validated(bytes, &profile);
    let guard = open_exact_revision(
        &request,
        &FakePort {
            bytes: bytes.to_vec(),
            digest: request.content_digest(),
            residency: request.residency().clone(),
        },
        CancellationToken::never(),
    )
    .expect("open");
    assert_eq!(guard.bytes(), bytes);
    assert_eq!(
        open_exact_revision(&request, &FailingPort, CancellationToken::never()),
        Err(MaterializationError::RevisionUnavailable)
    );
    let wrong_digest = FakePort {
        bytes: bytes.to_vec(),
        digest: Blake3Digest32::from_bytes([9; 32]),
        residency: request.residency().clone(),
    };
    assert_eq!(
        open_exact_revision(&request, &wrong_digest, CancellationToken::never()),
        Err(MaterializationError::RevisionDigestMismatch)
    );
    let wrong_residency = FakePort {
        bytes: bytes.to_vec(),
        digest: request.content_digest(),
        residency: OpaqueId::new("residency:other").expect("residency"),
    };
    assert_eq!(
        open_exact_revision(&request, &wrong_residency, CancellationToken::never()),
        Err(MaterializationError::ResidencyMismatch)
    );
}

#[test]
fn end_to_end_product_is_deterministic() {
    let first = materialize(b"same\nbytes\n");
    let second = materialize(b"same\nbytes\n");
    assert_eq!(first.representation_id(), second.representation_id());
    assert_eq!(first.canonical_digest(), second.canonical_digest());
    assert_eq!(
        canonicalize_materialization(&first)
            .expect("canonical")
            .as_slice(),
        canonicalize_materialization(&second)
            .expect("canonical")
            .as_slice()
    );
    let changed = materialize(b"same\nBYTES\n");
    assert_ne!(first.representation_id(), changed.representation_id());
}

#[test]
fn verify_proves_revision_and_profile_binding() {
    let profile = profile();
    let bytes = b"verify\nme\n".as_slice();
    let request = validated(bytes, &profile);
    let product = materialize(bytes);
    let receipt = verify_materialization(&product, &request, &profile).expect("verify");
    assert_eq!(receipt.representation_id(), product.representation_id());
    assert_eq!(receipt.assurance().ceiling(), product.assurance().ceiling());
    let other_profile = crate::profile::validate_materializer_profile(
        &baseline_profile_descriptor("other-verify", 1),
    )
    .expect("other");
    assert_eq!(
        verify_materialization(&product, &request, &other_profile),
        Err(MaterializationError::ProfileMismatch)
    );
}

#[test]
fn admission_plan_is_content_addressed_and_deadline_bound() {
    let product = materialize(b"plan\n");
    let operation = OpaqueId::new("operation:plan").expect("operation");
    let plan = prepare_admission(&product, &operation, 100).expect("plan");
    assert_eq!(plan.representation_id(), product.representation_id());
    assert_eq!(plan.canonical_digest(), product.canonical_digest());
    assert_eq!(
        prepare_admission(&product, &operation, 0),
        Err(MaterializationError::RequestInvalid)
    );
}

#[test]
fn resource_receipt_reports_bounded_work() {
    let product = materialize(b"work\nreport\n");
    let resource = product.resource_receipt();
    assert_eq!(resource.input_bytes, 12);
    assert_eq!(resource.output_bytes, 12);
    assert!(resource.steps_used > 0);
    assert_eq!(resource.segments, 1);
    assert_eq!(resource.loss_records, 0);
}

#[test]
fn debug_views_stay_content_free() {
    let product = materialize(b"top-secret-bytes\n");
    assert!(!format!("{product:?}").contains("top-secret-bytes"));
    let canonical = canonicalize_materialization(&product).expect("canonical");
    assert!(!format!("{canonical:?}").contains("top-secret"));
}

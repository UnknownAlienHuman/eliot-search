//! Baseline contract tests for `search-materializer` (W2/P04).
//!
//! These tests exercise the normative surface from `FUNCTIONS.md` through the
//! public entry module only: profile identity, encoding/BOM/newline policies,
//! coordinate/loss maps, loss/assurance taxonomy, deterministic identity,
//! bounded budgets, cancellation and the gated P17 provider seam.

use core::sync::atomic::{AtomicBool, Ordering};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};
use search_materializer::api::*;

fn opaque(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("opaque id")
}

fn test_profile() -> ValidatedMaterializerProfile {
    validate_materializer_profile(&baseline_profile_descriptor("test-baseline", 1))
        .expect("test profile validates")
}

const fn fake_port(bytes: Vec<u8>, digest: Blake3Digest32, residency: OpaqueId) -> FakePort {
    FakePort {
        bytes,
        digest,
        residency,
    }
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

fn digest_for(bytes: &[u8]) -> Blake3Digest32 {
    let mut out = [0_u8; 32];
    for (index, byte) in bytes.iter().enumerate() {
        let low = index.to_le_bytes()[0];
        out[index % 32] ^= byte.wrapping_add(low);
    }
    let len_low = bytes.len().to_le_bytes()[0];
    out[0] ^= len_low;
    Blake3Digest32::from_bytes(out)
}

fn byte_count(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).expect("byte count fits")
}

fn test_request(
    profile: &ValidatedMaterializerProfile,
    bytes: &[u8],
    encoding: SourceEncoding,
) -> ValidatedMaterializationRequest {
    let request = MaterializationRequest {
        source_id: opaque("source:test"),
        revision: NonZeroRevision::new(3).expect("revision"),
        residency: opaque("residency:test"),
        content_digest: digest_for(bytes),
        byte_count: byte_count(bytes),
        declared_kind: SourceKind::Text,
        declared_encoding: encoding,
        profile_id: profile_digest(profile),
        operation_id: opaque("operation:test"),
        from_unsaved_bytes: false,
        unsaved_snapshot_receipt: None,
    };
    let accepted = AcceptedProfiles::new(vec![profile.clone()]);
    validate_materialization_request(&request, &accepted, &DEFAULT_MATERIALIZATION_BUDGET)
        .expect("request validates")
}

fn materialize_bytes(bytes: Vec<u8>, encoding: SourceEncoding) -> MaterializationProduct {
    let profile = test_profile();
    let request = test_request(&profile, &bytes, encoding);
    let port = fake_port(bytes, request.content_digest(), request.residency().clone());
    let context = MaterializationContext {
        profile,
        budget: DEFAULT_MATERIALIZATION_BUDGET,
        cancel: CancellationToken::never(),
    };
    materialize_text_or_code(&request, &port, &context).expect("materializes")
}

#[test]
fn utf8_exact_golden_is_stable() {
    let product = materialize_bytes(b"hello\nworld\n".to_vec(), SourceEncoding::Utf8);
    assert_eq!(product.canonical_text(), "hello\nworld\n");
    assert_eq!(product.assurance().ceiling(), AssuranceCeiling::ExactBytes);
    assert!(product.maps().loss_map().records().is_empty());
    assert_eq!(product.maps().coordinate_map().segments().len(), 1);
    assert_eq!(
        product.maps().coordinate_map().segments()[0].relation,
        SegmentRelation::Exact
    );
}

#[test]
fn bom_is_stripped_with_recorded_loss() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"abc\n");
    let product = materialize_bytes(bytes, SourceEncoding::Utf8);
    assert_eq!(product.canonical_text(), "abc\n");
    assert_eq!(
        product.assurance().ceiling(),
        AssuranceCeiling::NormalizedWithRecordedLoss
    );
    assert_eq!(product.maps().loss_map().records().len(), 1);
    assert_eq!(
        product.maps().loss_map().records()[0].kind,
        LossKind::RemovedBom
    );
    assert!(
        product
            .maps()
            .coordinate_map()
            .segments()
            .iter()
            .any(|segment| segment.relation == SegmentRelation::Unmapped)
    );
}

#[test]
fn utf16_without_bom_transcodes_exactly() {
    let text = "A\n";
    let mut bytes = Vec::new();
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let product = materialize_bytes(bytes, SourceEncoding::Utf16Le);
    assert_eq!(product.canonical_text(), text);
    assert_eq!(
        product.assurance().ceiling(),
        AssuranceCeiling::ExactTranscoded
    );
}

#[test]
fn malformed_input_never_succeeds() {
    let profile = test_profile();
    // 0xFF alone admits no BOM reading: strictly malformed UTF-8.
    let bytes = vec![0xFF, 0x41];
    let request = test_request(&profile, &bytes, SourceEncoding::Utf8);
    let port = fake_port(bytes, request.content_digest(), request.residency().clone());
    let context = MaterializationContext {
        profile,
        budget: DEFAULT_MATERIALIZATION_BUDGET,
        cancel: CancellationToken::never(),
    };
    assert_eq!(
        materialize_text_or_code(&request, &port, &context),
        Err(MaterializationError::InvalidSequence)
    );
}

#[test]
fn over_limit_input_never_succeeds() {
    let profile = test_profile();
    let bytes = b"0123456789".to_vec();
    let request = test_request(&profile, &bytes, SourceEncoding::Utf8);
    let port = fake_port(bytes, request.content_digest(), request.residency().clone());
    let budget = MaterializationBudget {
        max_input_bytes: 2,
        ..DEFAULT_MATERIALIZATION_BUDGET
    };
    let context = MaterializationContext {
        profile,
        budget,
        cancel: CancellationToken::never(),
    };
    assert_eq!(
        materialize_text_or_code(&request, &port, &context),
        Err(MaterializationError::BudgetExhausted)
    );
}

#[test]
fn cancellation_never_returns_success() {
    let profile = test_profile();
    let bytes = b"line one\nline two\n".to_vec();
    let request = test_request(&profile, &bytes, SourceEncoding::Utf8);
    let port = fake_port(bytes, request.content_digest(), request.residency().clone());
    let flag = AtomicBool::new(true);
    let context = MaterializationContext {
        profile,
        budget: DEFAULT_MATERIALIZATION_BUDGET,
        cancel: CancellationToken::new(&flag),
    };
    assert_eq!(
        materialize_text_or_code(&request, &port, &context),
        Err(MaterializationError::Cancelled)
    );
    flag.store(false, Ordering::SeqCst);
}

#[test]
fn determinism_binds_revision_profile_encoding_bytes_maps() {
    let first = materialize_bytes(b"same\nbytes\n".to_vec(), SourceEncoding::Utf8);
    let second = materialize_bytes(b"same\nbytes\n".to_vec(), SourceEncoding::Utf8);
    assert_eq!(first.representation_id(), second.representation_id());
    assert_eq!(
        canonicalize_materialization(&first)
            .expect("canonical")
            .as_slice(),
        canonicalize_materialization(&second)
            .expect("canonical")
            .as_slice()
    );
    let other_revision = {
        let profile = test_profile();
        let bytes = b"same\nbytes\n".to_vec();
        let mut request = test_request(&profile, &bytes, SourceEncoding::Utf8);
        request = ValidatedMaterializationRequest::with_revision(
            request,
            NonZeroRevision::new(4).expect("revision"),
        );
        let port = fake_port(bytes, request.content_digest(), request.residency().clone());
        let context = MaterializationContext {
            profile,
            budget: DEFAULT_MATERIALIZATION_BUDGET,
            cancel: CancellationToken::never(),
        };
        materialize_text_or_code(&request, &port, &context).expect("materializes")
    };
    assert_ne!(
        first.representation_id(),
        other_revision.representation_id()
    );
}

#[test]
fn profile_change_classification_is_fail_closed() {
    let old = test_profile();
    let same = validate_materializer_profile(&baseline_profile_descriptor("test-baseline", 1))
        .expect("profile");
    assert_eq!(
        classify_profile_change(&old, &same),
        MaterializerProfileChange::Noop
    );
    let bumped = validate_materializer_profile(&baseline_profile_descriptor("test-baseline", 2))
        .expect("profile");
    // Same name but a changed golden digest implies changed behavior: reprepare.
    assert_eq!(
        classify_profile_change(&old, &bumped),
        MaterializerProfileChange::RePreparationAndReprojection
    );
    let downgrade =
        validate_materializer_profile(&baseline_profile_descriptor("other", 1)).expect("profile");
    assert_eq!(
        classify_profile_change(&old, &downgrade),
        MaterializerProfileChange::Reject
    );
}

#[test]
fn unsaved_bytes_require_admitted_snapshot() {
    let profile = test_profile();
    let bytes = b"unsaved\n".to_vec();
    let request = MaterializationRequest {
        source_id: opaque("source:test"),
        revision: NonZeroRevision::new(1).expect("revision"),
        residency: opaque("residency:test"),
        content_digest: digest_for(&bytes),
        byte_count: byte_count(&bytes),
        declared_kind: SourceKind::Text,
        declared_encoding: SourceEncoding::Utf8,
        profile_id: profile_digest(&profile),
        operation_id: opaque("operation:test"),
        from_unsaved_bytes: true,
        unsaved_snapshot_receipt: None,
    };
    let accepted = AcceptedProfiles::new(vec![profile]);
    assert_eq!(
        validate_materialization_request(&request, &accepted, &DEFAULT_MATERIALIZATION_BUDGET),
        Err(MaterializationError::UnsavedSnapshotNotAdmitted)
    );
    let admitted = MaterializationRequest {
        unsaved_snapshot_receipt: Some(ReceiptRef::new("receipt:snapshot").expect("receipt")),
        ..request
    };
    // Rebuild accepted set since `request` moved profile out via clone path.
    let profile = test_profile();
    let accepted = AcceptedProfiles::new(vec![profile]);
    validate_materialization_request(&admitted, &accepted, &DEFAULT_MATERIALIZATION_BUDGET)
        .expect("admitted snapshot validates");
}

#[test]
fn provider_seam_stays_gated() {
    let descriptor = ProviderDescriptor::new("pdf-worker".to_string());
    assert_eq!(
        validate_provider_descriptor(&descriptor),
        Err(MaterializationError::ProviderNotQualified)
    );
    assert_eq!(
        qualify_provider(&descriptor, &digest_for(b"fixtures")),
        Err(MaterializationError::ProviderNotQualified)
    );
    assert_eq!(
        classify_provider_failure(ProviderFailureKind::ProviderAbsent),
        ProviderFallbackDecision::UseBaselineTextCode
    );
    // Baseline text/code materialization works with the optional provider absent.
    let product = materialize_bytes(b"baseline works\n".to_vec(), SourceEncoding::Utf8);
    assert_eq!(product.assurance().ceiling(), AssuranceCeiling::ExactBytes);
}

#[test]
fn technical_views_never_leak_content() {
    let product = materialize_bytes(b"sensitive-source-bytes\n".to_vec(), SourceEncoding::Utf8);
    let debug = format!("{product:?}");
    assert!(!debug.contains("sensitive-source-bytes"));
    let canonical = canonicalize_materialization(&product).expect("canonical");
    assert!(!canonical.as_slice().windows(8).any(|w| w == b"sensitiv"));
    let receipt = verify_materialization(
        &product,
        &test_request(
            &test_profile(),
            b"sensitive-source-bytes\n",
            SourceEncoding::Utf8,
        ),
        &test_profile(),
    )
    .expect("verifies");
    assert!(!format!("{receipt:?}").contains("sensitive-source-bytes"));
}

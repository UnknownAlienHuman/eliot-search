//! Causal source fixtures AUTHORED_NOT_COMPILED/NOT_RUN; execution is #215/#216.
use super::budget::Meter;
use super::model::MarkdownUnitizationError as Error;
use super::*;
use crate::{
    UnitizationLimits, V3AnchorPolicy, V3AttachmentPolicy, V3EmptyPolicy, V3OmissionPolicy,
    V3OverlapPolicy, V3RepresentationKind, V3SourceBinding, V3UnitizerProfileDescriptor,
    ValidatedV3UnitizerProfile, validate_v3_unitizer_profile,
};
use search_contracts::{
    Blake3Digest32, CanonicalValue, MaterializationId, NativeAnchor, NonZeroRevision, OpaqueId,
    ProfileId, RepresentationId, SourceId, SourceNamespaceId, SourceRevisionId, UnitKind,
};
use search_materializer::api::{
    AcceptedProfiles, BomPolicy, CancellationToken, LossBehavior, MAX_MARKDOWN_PREIMAGE_BYTES,
    MAX_MARKDOWN_TOTAL_CANONICAL_BYTES, MarkdownLimits, MarkdownOperationLimits, MarkdownProduct,
    MarkdownProfile, MaterializationBudget, MaterializationContext, MaterializationError,
    MaterializationRequest, RevisionReadPort, SourceEncoding, SourceKind, StoredRevisionBytes,
    ValidatedMaterializationRequest, ValidatedMaterializerProfile, baseline_profile_descriptor,
    materialize_markdown, validate_materialization_request, validate_materializer_profile,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const INPUT: &str = "# Главная\r\n\r\n## Дочерняя\r\n\r\n> - **жирный** [ссылка](https://example.invalid/no-fetch \"заголовок\")\r\n\r\nкод \u{60}а&б\u{60} ![рисунок](urn:inert)\r\n\r\n~~~ rust\r\nnever_execute();\r\n~~~\r\n";
const RAW_BUDGET: MaterializationBudget = MaterializationBudget {
    max_input_bytes: 4096,
    max_output_bytes: 4096,
    max_lines: 512,
    max_map_segments: 512,
    max_loss_records: 1,
    max_steps: 65536,
};
const MD_LIMITS: MarkdownLimits = MarkdownLimits {
    input_bytes: 4096,
    next_attempts: 512,
    stack_depth: 32,
    nodes: 64,
    inline_records: 128,
    metadata_bytes_each: 512,
    metadata_bytes_total: 8192,
    owned_bytes: 65536,
    work_steps: 65536,
};
const REPLAY: MarkdownOperationLimits = MarkdownOperationLimits {
    parser: MD_LIMITS,
    canonical_preimage_bytes: MAX_MARKDOWN_PREIMAGE_BYTES,
    total_canonical_bytes: MAX_MARKDOWN_TOTAL_CANONICAL_BYTES,
};
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}
fn digest(bytes: &[u8]) -> Blake3Digest32 {
    Blake3Digest32::from_stored_bytes(*blake3::hash(bytes).as_bytes())
}
struct Port<'a> {
    bytes: Vec<u8>,
    source: OpaqueId,
    residency: OpaqueId,
    revision: NonZeroRevision,
    reads: AtomicUsize,
    cancel: &'a AtomicBool,
    action: AtomicUsize,
}
impl RevisionReadPort for Port<'_> {
    fn read_exact(
        &self,
        source: &OpaqueId,
        revision: NonZeroRevision,
        count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        if source != &self.source || revision != self.revision || count != self.bytes.len() as u64 {
            return Err(MaterializationError::RevisionUnavailable);
        }
        match self.action.load(Ordering::Relaxed) {
            1 => self.cancel.store(true, Ordering::Release),
            2 => std::thread::sleep(Duration::from_millis(20)),
            _ => {}
        }
        Ok(StoredRevisionBytes::new(
            self.bytes.clone(),
            digest(&self.bytes),
            self.residency.clone(),
        ))
    }
}
struct Fixture<'a> {
    binding: MarkdownSourceBinding,
    port: Port<'a>,
    request: ValidatedMaterializationRequest,
    raw: ValidatedMaterializerProfile,
    markdown: MarkdownProfile,
    product: MarkdownProduct,
    unitizer: ValidatedV3UnitizerProfile,
    profile: MarkdownUnitizerProfile,
}
impl<'a> Fixture<'a> {
    fn new(input: &str, flag: &'a AtomicBool) -> Self {
        let source = SourceId::from_bytes([2; 16]);
        let mut raw = baseline_profile_descriptor("440-raw", 1);
        raw.source_kinds = vec![SourceKind::Text];
        raw.encodings = vec![SourceEncoding::Utf8];
        raw.bom_policy = BomPolicy::RejectWhenPresent;
        raw.loss_behavior = LossBehavior::RejectOnAnyLoss;
        let raw = validate_materializer_profile(&raw).unwrap();
        let request = MaterializationRequest {
            source_id: OpaqueId::new(source.to_string()).unwrap(),
            revision: NonZeroRevision::new(1).unwrap(),
            residency: OpaqueId::new("440-residency").unwrap(),
            content_digest: digest(input.as_bytes()),
            byte_count: input.len() as u64,
            declared_kind: SourceKind::Text,
            declared_encoding: SourceEncoding::Utf8,
            profile_id: raw.id(),
            operation_id: OpaqueId::new("440-materialize").unwrap(),
            from_unsaved_bytes: false,
            unsaved_snapshot_receipt: None,
        };
        let request = validate_materialization_request(
            &request,
            &AcceptedProfiles::new(vec![raw.clone()]),
            &RAW_BUDGET,
        )
        .unwrap();
        let port = Port {
            bytes: input.as_bytes().to_vec(),
            source: request.source_id().clone(),
            residency: request.residency().clone(),
            revision: request.revision(),
            reads: AtomicUsize::new(0),
            cancel: flag,
            action: AtomicUsize::new(0),
        };
        let markdown = MarkdownProfile::new(&raw, MD_LIMITS).unwrap();
        let context = MaterializationContext {
            profile: raw.clone(),
            budget: RAW_BUDGET,
            cancel: CancellationToken::new(flag),
        };
        let product =
            materialize_markdown(&request, &port, &context, &markdown, REPLAY, deadline()).unwrap();
        let unitizer = validate_v3_unitizer_profile(&V3UnitizerProfileDescriptor {
            profile_name: ProfileId::new("440-raw-v3").unwrap(),
            profile_revision: 1,
            representation_kind: V3RepresentationKind::Text,
            unit_kind: UnitKind::Section,
            boundary_revision: 1,
            overlap_policy: V3OverlapPolicy::None,
            anchor_policy: V3AnchorPolicy::ExactTextBytes,
            attachment_policy: V3AttachmentPolicy::Absent,
            omission_policy: V3OmissionPolicy::Forbidden,
            empty_policy: V3EmptyPolicy::Reject,
            limits: UnitizationLimits {
                max_input_bytes: 4096,
                preferred_unit_bytes: 64,
                max_unit_bytes: 128,
                max_lines: 512,
                max_units: 128,
            },
            min_unit_bytes: 1,
            max_unit_scalars: 128,
            max_unit_lines: 128,
            max_anchor_depth: 1,
            max_manifest_bytes: 65536,
            max_steps: 65536,
            fixture_digest: Blake3Digest32::from_stored_bytes([7; 32]),
        })
        .unwrap();
        let binding = MarkdownSourceBinding::new(
            V3SourceBinding {
                source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
                source_id: source,
                source_revision_id: SourceRevisionId::from_bytes([3; 16]),
                materialization_id: MaterializationId::from_bytes([4; 16]),
                representation_id: RepresentationId::from_bytes([5; 16]),
            },
            RepresentationId::from_bytes([6; 16]),
        )
        .unwrap();
        let profile = MarkdownUnitizerProfile::new(
            unitizer.id().clone(),
            markdown.id(),
            MAX_MARKDOWN_UNITIZER_LIMITS,
        )
        .unwrap();
        Self {
            binding,
            port,
            request,
            raw,
            markdown,
            product,
            unitizer,
            profile,
        }
    }
    fn source(&self) -> MarkdownUnitizationSource<'_> {
        MarkdownUnitizationSource {
            binding: &self.binding,
            product: &self.product,
            request: &self.request,
            port: &self.port,
            raw_profile: &self.raw,
            raw_budget: RAW_BUDGET,
            markdown_profile: &self.markdown,
            raw_unitizer_profile: &self.unitizer,
            replay_limits: REPLAY,
        }
    }
    fn budget(&self) -> MarkdownUnitizationBudget<'a> {
        MarkdownUnitizationBudget {
            limits: MAX_MARKDOWN_UNITIZER_LIMITS,
            cancelled: self.port.cancel,
            deadline: deadline(),
        }
    }
    fn set(&self) -> VerifiedMarkdownUnitSet {
        unitize_markdown(&self.source(), &self.profile, &self.budget()).unwrap()
    }
}
fn reseal(
    manifest: &mut MarkdownUnitManifest,
    profile: &MarkdownUnitizerProfile,
    budget: &MarkdownUnitizationBudget<'_>,
) {
    let mut meter = Meter::new(profile, budget).unwrap();
    let context = super::build::context(&manifest.body, &mut meter).unwrap();
    for unit in &mut manifest.body.nodes {
        unit.identity = super::build::identity(context, &unit.data, &mut meter).unwrap();
        unit.occurrence = super::build::occurrence(
            super::build::truncated(unit.identity),
            manifest.body.structural,
            unit.data.ordinal,
            unit.data.kind,
            (unit.data.start, unit.data.end),
            manifest.body.raw.provenance().input_digest(),
            unit.identity,
        )
        .unwrap();
    }
    let body = super::codec::body(&manifest.body, &mut meter).unwrap();
    manifest.digest = meter.hash("eliot/cbor/md-unit-manifest/v4", body).unwrap();
}
#[test]
fn full_inventory_raw_coverage_crlf_utf8_and_mapped_values() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    let set = fixture.set();
    let manifest = set.manifest();
    assert_eq!(manifest.raw_manifest().input_bytes(), INPUT.len() as u64);
    assert_eq!(manifest.raw_manifest().emitted_bytes(), INPUT.len() as u64);
    assert_eq!(manifest.raw_manifest().omitted_bytes(), 0);
    assert_eq!(manifest.units().len(), fixture.product.nodes().len());
    assert_eq!(
        manifest.inline_observations().len(),
        fixture.product.inline_records().len()
    );
    assert_eq!(manifest.units()[0].occurrence().unit_kind, UnitKind::File);
    assert_eq!(set.representation().unit_manifest_digest, manifest.digest());
    assert_ne!(
        set.representation().representation_id,
        fixture.binding.raw().representation_id
    );
    assert!(
        manifest
            .units()
            .windows(2)
            .any(|pair| pair[0].start() <= pair[1].start() && pair[0].end() >= pair[1].end())
    );
    for unit in manifest.units() {
        assert!(
            INPUT
                .get(unit.start() as usize..unit.end() as usize)
                .is_some()
        );
        let NativeAnchor::TextBytes(anchor) = &unit.occurrence().native_anchor else {
            panic!("raw anchor");
        };
        assert_eq!(
            anchor.content_digest,
            fixture.product.raw_product().input_digest()
        );
        assert_eq!(
            (anchor.byte_start_0, anchor.byte_end_exclusive_0),
            (unit.start(), unit.end())
        );
    }
    assert!(manifest.inline_observations().iter().any(|r| {
        r.destination()
            .is_some_and(|v| v.value() == "https://example.invalid/no-fetch")
    }));
    assert!(manifest.inline_observations().iter().any(|r| {
        r.text()
            .is_some_and(|v| v.value() == "а&б" && v.assurance() == "mapped-text")
    }));
    assert_eq!(
        manifest.inline_metadata_status(),
        "observed-spans-and-selected-values"
    );
    assert_eq!(
        manifest.reference_metadata_status(),
        "potentially-incomplete"
    );
}
#[test]
fn nested_equal_extents_never_inflate_raw_coverage() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new("> - same\n", &flag);
    let set = fixture.set();
    let units = set.manifest().units();
    assert!(
        units.iter().enumerate().any(|(i, a)| units[i + 1..]
            .iter()
            .any(|b| (a.start(), a.end()) == (b.start(), b.end())))
    );
    assert!(
        units.iter().map(|u| u.end() - u.start()).sum::<u64>()
            > fixture.product.canonical_text().len() as u64
    );
    assert_eq!(
        set.manifest().raw_manifest().emitted_bytes(),
        fixture.product.canonical_text().len() as u64
    );
}
#[test]
fn canonical_roundtrip_stays_a_proposal_until_owning_replay() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    let set = fixture.set();
    let bytes =
        canonicalize_markdown_unit_manifest(set.manifest(), &fixture.profile, &fixture.budget())
            .unwrap();
    let proposed =
        decode_markdown_unit_manifest(bytes.as_slice(), &fixture.profile, &fixture.budget())
            .unwrap();
    assert_eq!(&proposed, set.manifest());
    let reads = fixture.port.reads.load(Ordering::Relaxed);
    verify_markdown_unit_manifest(
        &proposed,
        &fixture.source(),
        &fixture.profile,
        &fixture.budget(),
    )
    .unwrap();
    assert!(fixture.port.reads.load(Ordering::Relaxed) > reads);
}
#[test]
fn rehashed_root_only_and_missing_inline_refuse_completeness() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    let mut proposed = fixture.set().manifest;
    proposed.body.nodes.truncate(1);
    proposed.body.inline.clear();
    reseal(&mut proposed, &fixture.profile, &fixture.budget());
    canonicalize_markdown_unit_manifest(&proposed, &fixture.profile, &fixture.budget()).unwrap();
    assert!(matches!(
        verify_markdown_unit_manifest(
            &proposed,
            &fixture.source(),
            &fixture.profile,
            &fixture.budget()
        ),
        Err(Error::ManifestMismatch)
    ));
    let mut proposed = fixture.set().manifest;
    proposed.body.inline.pop().unwrap();
    reseal(&mut proposed, &fixture.profile, &fixture.budget());
    assert!(matches!(
        verify_markdown_unit_manifest(
            &proposed,
            &fixture.source(),
            &fixture.profile,
            &fixture.budget()
        ),
        Err(Error::ManifestMismatch)
    ));
}
#[test]
fn rehashed_parent_and_selected_value_mutations_refuse() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    let mut proposed = fixture.set().manifest;
    let child = proposed
        .body
        .nodes
        .iter_mut()
        .find(|u| u.data.parent_section.is_some())
        .unwrap();
    child.data.parent_section = None;
    reseal(&mut proposed, &fixture.profile, &fixture.budget());
    assert!(matches!(
        verify_markdown_unit_manifest(
            &proposed,
            &fixture.source(),
            &fixture.profile,
            &fixture.budget()
        ),
        Err(Error::ManifestMismatch)
    ));
    let mut proposed = fixture.set().manifest;
    let link = proposed
        .body
        .inline
        .iter_mut()
        .find(|r| r.destination.is_some())
        .unwrap();
    link.destination.as_mut().unwrap().value = "urn:tampered".into();
    reseal(&mut proposed, &fixture.profile, &fixture.budget());
    assert!(matches!(
        verify_markdown_unit_manifest(
            &proposed,
            &fixture.source(),
            &fixture.profile,
            &fixture.budget()
        ),
        Err(Error::ManifestMismatch)
    ));
}
fn wire_fields(bytes: &[u8]) -> Vec<CanonicalValue> {
    let CanonicalValue::Array(fields) = search_contracts::parse_canonical_cbor(bytes).unwrap()
    else {
        panic!("array");
    };
    fields.into_vec()
}
fn reject_wire(fields: Vec<CanonicalValue>, fixture: &Fixture<'_>) {
    let bytes = search_contracts::to_canonical_cbor(&super::codec::array(fields).unwrap()).unwrap();
    assert!(
        decode_markdown_unit_manifest(bytes.as_slice(), &fixture.profile, &fixture.budget())
            .is_err()
    );
}
#[test]
fn wrong_version_type_length_unknown_status_and_truncation_refuse() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    let set = fixture.set();
    let bytes =
        canonicalize_markdown_unit_manifest(set.manifest(), &fixture.profile, &fixture.budget())
            .unwrap();
    let mut fields = wire_fields(bytes.as_slice());
    fields[1] = CanonicalValue::U64(3);
    reject_wire(fields, &fixture);
    let mut fields = wire_fields(bytes.as_slice());
    fields[1] = CanonicalValue::Bool(true);
    reject_wire(fields, &fixture);
    let mut fields = wire_fields(bytes.as_slice());
    fields.push(CanonicalValue::Null);
    reject_wire(fields, &fixture);
    for slot in [8, 9] {
        let mut fields = wire_fields(bytes.as_slice());
        let CanonicalValue::Array(body) = std::mem::replace(&mut fields[2], CanonicalValue::Null)
        else {
            panic!("body");
        };
        let mut body = body.into_vec();
        body[slot] = super::codec::text("unknown-status").unwrap();
        let mut meter = Meter::new(&fixture.profile, &fixture.budget()).unwrap();
        let changed = super::codec::array(body).unwrap();
        fields[3] = super::codec::bytes(
            meter
                .hash("eliot/cbor/md-unit-manifest/v4", changed.clone())
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        fields[2] = changed;
        reject_wire(fields, &fixture);
    }
    assert!(
        decode_markdown_unit_manifest(
            &bytes.as_slice()[..bytes.len() - 1],
            &fixture.profile,
            &fixture.budget()
        )
        .is_err()
    );
}
#[test]
fn source_profile_and_distinct_admitted_binding_mismatch_refuse() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    assert!(matches!(
        MarkdownSourceBinding::new(
            fixture.binding.raw().clone(),
            fixture.binding.raw().representation_id
        ),
        Err(Error::BindingMismatch)
    ));
    let mut raw = fixture.binding.raw().clone();
    raw.source_id = SourceId::from_bytes([9; 16]);
    let wrong =
        MarkdownSourceBinding::new(raw, fixture.binding.structural_representation_id()).unwrap();
    let source = MarkdownUnitizationSource {
        binding: &wrong,
        ..fixture.source()
    };
    let reads = fixture.port.reads.load(Ordering::Relaxed);
    assert!(matches!(
        unitize_markdown(&source, &fixture.profile, &fixture.budget()),
        Err(Error::BindingMismatch)
    ));
    assert_eq!(fixture.port.reads.load(Ordering::Relaxed), reads);
    let wrong = MarkdownUnitizerProfile::new(
        ProfileId::new("wrong-raw-profile").unwrap(),
        fixture.markdown.id(),
        MAX_MARKDOWN_UNITIZER_LIMITS,
    )
    .unwrap();
    assert!(matches!(
        unitize_markdown(&fixture.source(), &wrong, &fixture.budget()),
        Err(Error::ProfileMismatch)
    ));
}
#[test]
fn full_digest_guard_rejects_injected_raw_structural_prefix_collision() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    let set = fixture.set();
    let raw = &set.manifest().raw_manifest().units()[0];
    let mut ids = BTreeMap::new();
    super::build::record(&mut ids, raw.unit_id(), raw.identity_digest()).unwrap();
    let mut full = *raw.identity_digest().as_bytes();
    full[31] ^= 1;
    let conflicting = Blake3Digest32::from_stored_bytes(full);
    assert_eq!(super::build::truncated(conflicting), raw.unit_id());
    assert_eq!(
        super::build::record(&mut ids, raw.unit_id(), conflicting),
        Err(Error::IdentityCollision)
    );
}
#[test]
fn zero_widening_and_cumulative_work_storage_canonical_limits_refuse() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    let set = fixture.set();
    let mut budget = fixture.budget();
    budget.limits.nodes = 0;
    assert!(matches!(
        unitize_markdown(&fixture.source(), &fixture.profile, &budget),
        Err(Error::InvalidLimits)
    ));
    budget = fixture.budget();
    budget.limits.work_steps += 1;
    assert!(matches!(
        unitize_markdown(&fixture.source(), &fixture.profile, &budget),
        Err(Error::InvalidLimits)
    ));
    for dimension in 0..3 {
        let mut budget = fixture.budget();
        match dimension {
            0 => budget.limits.work_steps = set.usage().work - 1,
            1 => budget.limits.owned_bytes = set.usage().owned_bytes - 1,
            _ => budget.limits.canonical_bytes = set.usage().canonical_bytes - 1,
        }
        assert!(unitize_markdown(&fixture.source(), &fixture.profile, &budget).is_err());
    }
}
#[test]
fn independent_raw_work_is_reserved_before_any_revision_read() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    let mut budget = fixture.budget();
    budget.limits.work_steps = MD_LIMITS.work_steps + 64;
    let reads = fixture.port.reads.load(Ordering::Relaxed);
    assert!(matches!(
        unitize_markdown(&fixture.source(), &fixture.profile, &budget),
        Err(Error::WorkLimit)
    ));
    assert_eq!(fixture.port.reads.load(Ordering::Relaxed), reads);
}
#[test]
fn original_cancel_and_deadline_survive_source_port_and_terminal_return() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new(INPUT, &flag);
    flag.store(true, Ordering::Release);
    let reads = fixture.port.reads.load(Ordering::Relaxed);
    assert!(matches!(
        unitize_markdown(&fixture.source(), &fixture.profile, &fixture.budget()),
        Err(Error::Cancelled)
    ));
    assert_eq!(fixture.port.reads.load(Ordering::Relaxed), reads);
    flag.store(false, Ordering::Release);
    let mut budget = fixture.budget();
    budget.deadline = Instant::now();
    assert!(matches!(
        unitize_markdown(&fixture.source(), &fixture.profile, &budget),
        Err(Error::DeadlineExceeded)
    ));
    fixture.port.action.store(1, Ordering::Relaxed);
    assert!(matches!(
        unitize_markdown(&fixture.source(), &fixture.profile, &fixture.budget()),
        Err(Error::Cancelled)
    ));
    assert!(fixture.port.reads.load(Ordering::Relaxed) > reads);
    flag.store(false, Ordering::Release);
    fixture.port.action.store(2, Ordering::Relaxed);
    budget = fixture.budget();
    budget.deadline = Instant::now() + Duration::from_millis(10);
    assert!(matches!(
        unitize_markdown(&fixture.source(), &fixture.profile, &budget),
        Err(Error::DeadlineExceeded)
    ));
}
#[test]
fn zero_width_inline_and_content_free_debug_keep_assurance() {
    let flag = AtomicBool::new(false);
    let fixture = Fixture::new("x\n\ny\n", &flag);
    let mut proposed = fixture.set().manifest;
    let record = &mut proposed.body.inline[0];
    record.end = record.start;
    reseal(&mut proposed, &fixture.profile, &fixture.budget());
    let bytes = canonicalize_markdown_unit_manifest(&proposed, &fixture.profile, &fixture.budget())
        .unwrap();
    let decoded =
        decode_markdown_unit_manifest(bytes.as_slice(), &fixture.profile, &fixture.budget())
            .unwrap();
    assert_eq!(
        decoded.inline_observations()[0].start(),
        decoded.inline_observations()[0].end()
    );
    assert!(matches!(
        verify_markdown_unit_manifest(
            &decoded,
            &fixture.source(),
            &fixture.profile,
            &fixture.budget()
        ),
        Err(Error::ManifestMismatch)
    ));
    let fixture = Fixture::new(INPUT, &flag);
    let set = fixture.set();
    let debug = format!("{:?}", set);
    assert!(!debug.contains("no-fetch") && !debug.contains("а&б") && !debug.contains("Главная"));
    assert_eq!(Error::Cancelled.to_string(), "MD_UNITIZER_CANCELLED");
}

//! Causal source fixtures AUTHORED_NOT_COMPILED/NOT_RUN; execution remains #215/#216.
use super::budget::Meter;
use super::model::JatsUnitizationError as Error;
use super::*;
use crate::{
    UnitizationLimits, V3AnchorPolicy, V3AttachmentPolicy, V3EmptyPolicy, V3OmissionPolicy,
    V3OverlapPolicy, V3RepresentationKind, V3SourceBinding, V3UnitizerProfileDescriptor,
    ValidatedV3UnitizerProfile, validate_v3_unitizer_profile,
};
use search_contracts::{
    Blake3Digest32, MaterializationId, NonZeroRevision, OpaqueId, ProfileId, RepresentationId,
    SourceId, SourceNamespaceId, SourceRevisionId, UnitKind,
};
use search_materializer::api::{
    AcceptedProfiles, BomPolicy, CancellationToken, JatsOperationLimits, JatsProduct, JatsProfile,
    LossBehavior, MAX_JATS_LIMITS, MAX_JATS_PREIMAGE_BYTES, MAX_JATS_TOTAL_CANONICAL_BYTES,
    MaterializationBudget, MaterializationContext, MaterializationError, MaterializationRequest,
    RevisionReadPort, SourceEncoding, SourceKind, StoredRevisionBytes,
    ValidatedMaterializationRequest, ValidatedMaterializerProfile, baseline_profile_descriptor,
    materialize_jats, validate_materialization_request, validate_materializer_profile,
};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const FRONT: &str = "<front><journal-meta><journal-id>J</journal-id><issn>1234</issn></journal-meta><article-meta><title-group><article-title>Тест</article-title></title-group></article-meta></front>";
const RAW_BUDGET: MaterializationBudget = MaterializationBudget {
    max_input_bytes: 1024 * 1024,
    max_output_bytes: 1024 * 1024,
    max_lines: 4096,
    max_map_segments: 4096,
    max_loss_records: 1,
    max_steps: 16 * 1024 * 1024,
};
const REPLAY: JatsOperationLimits = JatsOperationLimits {
    parser: MAX_JATS_LIMITS,
    canonical_preimage_bytes: MAX_JATS_PREIMAGE_BYTES,
    total_canonical_bytes: MAX_JATS_TOTAL_CANONICAL_BYTES,
};
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}
fn digest(bytes: &[u8]) -> Blake3Digest32 {
    Blake3Digest32::from_stored_bytes(*blake3::hash(bytes).as_bytes())
}
fn article(body: &str) -> String {
    format!("<article>{FRONT}<body>{body}</body></article>")
}
fn specimen() -> String {
    format!(
        "<?xml version=\"1.0\"?><!--before--><article xmlns:xlink=\"http://www.w3.org/1999/xlink\" id=\"private-id\">{FRONT}<body><sec id=\"s\"><title>T</title><p>A\r\nЖ&amp;<![CDATA[x]]><italic>Y</italic><xref rid=\"r\">R</xref><ext-link xlink:href=\"urn:private-destination\">L</ext-link></p><!--inside--><fig><caption><p>F</p></caption></fig><table-wrap><table><tbody><tr><td>C</td></tr></tbody></table></table-wrap></sec><p/></body><back><ref-list><ref id=\"r\"><mixed-citation><article-title>Citation</article-title></mixed-citation></ref></ref-list></back></article><!--after-->"
    )
}
thread_local! {static HOOK:Cell<(bool,u8,usize)>=const{Cell::new((false,0,0))};}
pub(super) fn terminal(meter: &Meter<'_>, verify: bool) {
    HOOK.with(|hook| {
        let (phase, action, hits) = hook.get();
        if action == 0 || phase != verify {
            return;
        }
        hook.set((phase, action, hits + 1));
        if action == 1 {
            meter.budget.cancelled.store(true, Ordering::Release);
        }
        if action == 2 {
            while Instant::now() < meter.budget.deadline {
                std::thread::yield_now();
            }
        }
    });
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
    binding: JatsSourceBinding,
    port: Port<'a>,
    request: ValidatedMaterializationRequest,
    raw: ValidatedMaterializerProfile,
    jats: JatsProfile,
    product: JatsProduct,
    unitizer: ValidatedV3UnitizerProfile,
    profile: JatsUnitizerProfile,
}
impl<'a> Fixture<'a> {
    fn new(input: &str, flag: &'a AtomicBool) -> Self {
        let source = SourceId::from_bytes([2; 16]);
        let mut raw = baseline_profile_descriptor("jats-v5-raw", 1);
        raw.source_kinds = vec![SourceKind::Text];
        raw.encodings = vec![SourceEncoding::Utf8];
        raw.bom_policy = BomPolicy::RejectWhenPresent;
        raw.loss_behavior = LossBehavior::RejectOnAnyLoss;
        let raw = validate_materializer_profile(&raw).unwrap();
        let request = MaterializationRequest {
            source_id: OpaqueId::new(source.to_string()).unwrap(),
            revision: NonZeroRevision::new(1).unwrap(),
            residency: OpaqueId::new("jats-v5-residency").unwrap(),
            content_digest: digest(input.as_bytes()),
            byte_count: input.len() as u64,
            declared_kind: SourceKind::Text,
            declared_encoding: SourceEncoding::Utf8,
            profile_id: raw.id(),
            operation_id: OpaqueId::new("jats-v5-materialize").unwrap(),
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
        let jats = JatsProfile::new(&raw, MAX_JATS_LIMITS).unwrap();
        let context = MaterializationContext {
            profile: raw.clone(),
            budget: RAW_BUDGET,
            cancel: CancellationToken::new(flag),
        };
        let product =
            materialize_jats(&request, &port, &context, &jats, REPLAY, deadline()).unwrap();
        let unitizer = validate_v3_unitizer_profile(&V3UnitizerProfileDescriptor {
            profile_name: ProfileId::new("jats-v5-raw-v3").unwrap(),
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
                max_input_bytes: 1024 * 1024,
                preferred_unit_bytes: 4096,
                max_unit_bytes: 8192,
                max_lines: 4096,
                max_units: 4096,
            },
            min_unit_bytes: 1,
            max_unit_scalars: 8192,
            max_unit_lines: 4096,
            max_anchor_depth: 1,
            max_manifest_bytes: 1024 * 1024,
            max_steps: 32_000_000,
            fixture_digest: Blake3Digest32::from_stored_bytes([7; 32]),
        })
        .unwrap();
        let binding = JatsSourceBinding::new(
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
        let profile =
            JatsUnitizerProfile::new(unitizer.id().clone(), jats.id(), MAX_JATS_UNITIZER_LIMITS)
                .unwrap();
        Self {
            binding,
            port,
            request,
            raw,
            jats,
            product,
            unitizer,
            profile,
        }
    }
    fn source(&self) -> JatsUnitizationSource<'_> {
        JatsUnitizationSource {
            binding: &self.binding,
            product: &self.product,
            request: &self.request,
            port: &self.port,
            raw_profile: &self.raw,
            raw_budget: RAW_BUDGET,
            jats_profile: &self.jats,
            raw_unitizer_profile: &self.unitizer,
            replay_limits: REPLAY,
        }
    }
    fn budget(&self) -> JatsUnitizationBudget<'_> {
        JatsUnitizationBudget {
            limits: MAX_JATS_UNITIZER_LIMITS,
            cancelled: self.port.cancel,
            deadline: deadline(),
        }
    }
    fn set(&self) -> VerifiedJatsUnitSet {
        unitize_jats(&self.source(), &self.profile, &self.budget()).unwrap()
    }
}
fn reseal(m: &mut JatsUnitManifest, p: &JatsUnitizerProfile, b: &JatsUnitizationBudget<'_>) {
    let mut meter = Meter::new(p, b).unwrap();
    let ctx = super::build::context(&m.body, &mut meter).unwrap();
    for unit in &mut m.body.nodes {
        unit.identity = super::build::identity(ctx, &unit.data, &mut meter).unwrap();
        unit.occurrence = super::build::occurrence(
            super::build::truncated(unit.identity),
            m.body.structural,
            &unit.data,
            m.body.raw.provenance().input_digest(),
            unit.identity,
        )
        .unwrap();
    }
    let value = super::codec::body(&m.body, &mut meter).unwrap();
    m.digest = meter
        .hash("eliot/cbor/jats-unit-manifest/v5", value)
        .unwrap();
}
#[test]
fn full_inventory_original_ranges_raw_denominator_and_codec() {
    let flag = AtomicBool::new(false);
    let f = Fixture::new(&specimen(), &flag);
    let set = f.set();
    let m = set.manifest();
    assert_eq!(m.units().len(), f.product.elements().len());
    assert_eq!(m.attributes().len(), f.product.attributes().len());
    assert_eq!(m.texts().len(), f.product.texts().len());
    assert_eq!(m.comments().len(), 3);
    assert_eq!(m.event_calls(), f.product.event_calls() as u64);
    assert_eq!(m.declaration_span().unwrap().start(), 0);
    assert!(m.units()[0].span().start() > 0);
    assert_eq!(m.units()[0].kind(), "article");
    for (i, (unit, node)) in m.units().iter().zip(f.product.elements()).enumerate() {
        assert_eq!(unit.occurrence().ordinal, i as u64);
        assert_eq!(unit.kind(), node.kind().name());
        assert_eq!(unit.parent(), node.parent().map(|n| n as u64));
        assert_eq!(unit.event(), node.event() as u64);
        assert_eq!(unit.close_event(), node.close_event() as u64);
        assert_eq!(unit.span().start(), node.span().start() as u64);
        assert_eq!(unit.span().end(), node.span().end() as u64);
        assert_eq!(unit.opening().end(), node.opening().end() as u64);
        assert_eq!(
            unit.closing().map(|s| (s.start(), s.end())),
            node.closing().map(|s| (s.start() as u64, s.end() as u64))
        );
    }
    assert!(
        m.texts()
            .iter()
            .any(|t| t.kind() == "reference" && t.value().value() == "&")
    );
    assert!(
        m.texts()
            .iter()
            .any(|t| t.kind() == "cdata" && t.value().value() == "x")
    );
    let raw_budget = crate::UnitizationBudget {
        max_steps: f.unitizer.descriptor().max_steps,
        max_encoded_bytes: f.unitizer.descriptor().max_manifest_bytes,
        deadline: deadline(),
        cancelled: &flag,
    };
    let input = crate::prepare_unit_set_input(
        f.binding.raw().clone(),
        f.product.raw_product(),
        &f.request,
        &f.raw,
        &f.unitizer,
        &raw_budget,
    )
    .unwrap();
    let raw = crate::build_unit_manifest(&input, &f.unitizer, &raw_budget).unwrap();
    assert_eq!(&raw.manifest, m.raw_manifest());
    assert_eq!(
        m.raw_manifest().emitted_bytes(),
        f.product.canonical_text().len() as u64
    );
    let encoded = canonicalize_jats_unit_manifest(m, &f.profile, &f.budget()).unwrap();
    let decoded = decode_jats_unit_manifest(encoded.as_slice(), &f.profile, &f.budget()).unwrap();
    assert_eq!(&decoded, m);
    assert_eq!(
        verify_jats_unit_manifest(&decoded, &f.source(), &f.profile, &f.budget())
            .unwrap()
            .manifest(),
        m
    );
}
#[test]
fn zero_event_article_without_declaration_and_empty_syntax() {
    let flag = AtomicBool::new(false);
    let f = Fixture::new(&article("<p/>"), &flag);
    let set = f.set();
    assert_eq!(set.manifest().units()[0].event(), 0);
    let last = set.manifest().units().last().unwrap();
    assert_eq!(last.kind(), "p");
    assert_eq!(last.span(), last.opening());
    assert!(last.closing().is_none());
    assert_eq!(last.event(), last.close_event());
    let f = Fixture::new(&format!("<!--before-->{}", article("<p/>")), &flag);
    let set = f.set();
    let m = set.manifest();
    assert_eq!(m.comments()[0].event(), 0);
    assert_eq!(m.comments()[0].owner(), None);
    let bytes = canonicalize_jats_unit_manifest(m, &f.profile, &f.budget()).unwrap();
    let decoded = decode_jats_unit_manifest(bytes.as_slice(), &f.profile, &f.budget()).unwrap();
    assert_eq!(&decoded, m);
    assert_eq!(
        verify_jats_unit_manifest(&decoded, &f.source(), &f.profile, &f.budget())
            .unwrap()
            .manifest(),
        m
    );
}
#[test]
fn rehashed_shape_valid_omissions_and_changes_reach_owning_replay() {
    let flag = AtomicBool::new(false);
    let f = Fixture::new(&specimen(), &flag);
    for mutation in 0..9 {
        let mut m = f.set().manifest;
        match mutation {
            0 => {
                m.body.attributes.pop();
            }
            1 => {
                m.body.texts.pop();
            }
            2 => {
                m.body.comments.pop();
            }
            3 => m.body.declaration = None,
            4 => m.body.attributes[0].value.value.push('x'),
            5 => m.body.texts[0].value.value.push('x'),
            6 => m.body.events += 1,
            7 => {
                let index = m
                    .body
                    .nodes
                    .iter()
                    .position(|u| u.kind() == "italic")
                    .unwrap();
                m.body.nodes[index].data.parent =
                    Some(m.body.nodes.iter().position(|u| u.kind() == "sec").unwrap() as u64);
            }
            _ => {
                m.body.nodes.last_mut().unwrap().data.content =
                    Blake3Digest32::from_stored_bytes([9; 32]);
            }
        }
        reseal(&mut m, &f.profile, &f.budget());
        let before = f.port.reads.load(Ordering::Relaxed);
        assert_eq!(
            verify_jats_unit_manifest(&m, &f.source(), &f.profile, &f.budget()).unwrap_err(),
            Error::ManifestMismatch
        );
        assert_eq!(f.port.reads.load(Ordering::Relaxed), before + 1);
    }
    let f = Fixture::new(&article("<p>kept</p><p/>"), &flag);
    let mut m = f.set().manifest;
    m.body.nodes.pop();
    reseal(&mut m, &f.profile, &f.budget());
    let before = f.port.reads.load(Ordering::Relaxed);
    assert_eq!(
        verify_jats_unit_manifest(&m, &f.source(), &f.profile, &f.budget()).unwrap_err(),
        Error::ManifestMismatch
    );
    assert_eq!(f.port.reads.load(Ordering::Relaxed), before + 1);
}
#[test]
fn malformed_node_order_and_wire_refuse() {
    let flag = AtomicBool::new(false);
    let f = Fixture::new(&specimen(), &flag);
    let mut m = f.set().manifest;
    m.body.nodes.swap(1, 2);
    assert_eq!(
        verify_jats_unit_manifest(&m, &f.source(), &f.profile, &f.budget()).unwrap_err(),
        Error::InvalidStructure
    );
    let encoded =
        canonicalize_jats_unit_manifest(f.set().manifest(), &f.profile, &f.budget()).unwrap();
    let mut bytes = encoded.as_slice().to_vec();
    bytes.push(0);
    assert!(decode_jats_unit_manifest(&bytes, &f.profile, &f.budget()).is_err());
    for n in [0, encoded.len() - 1] {
        assert!(
            decode_jats_unit_manifest(&encoded.as_slice()[..n], &f.profile, &f.budget()).is_err()
        );
    }
}
#[test]
fn separate_large_attribute_and_text_inventories_preserve_all_chunks() {
    let flag = AtomicBool::new(false);
    for body in [
        "<p id='a' xml:lang='en' xml:space='default' content-type='x' specific-use='x'/>"
            .repeat(821),
        format!("<p>{}</p>", "x&amp;".repeat(2049)),
    ] {
        let f = Fixture::new(&article(&body), &flag);
        let set = f.set();
        let m = set.manifest();
        assert!(m.attributes().len() > 4096 || m.texts().len() > 4096);
        if m.attributes().len() > 4096 {
            assert_eq!(m.attributes().len(), 4105);
        } else {
            let paragraph = m.units().iter().position(|u| u.kind() == "p").unwrap() as u64;
            let texts: Vec<_> = m
                .texts()
                .iter()
                .filter(|t| t.owner() == paragraph)
                .collect();
            assert_eq!(m.texts().len(), 4101);
            assert_eq!(texts.len(), 4098);
            for pair in texts.chunks_exact(2) {
                assert_eq!(pair[0].kind(), "text");
                assert_eq!(pair[0].value().value(), "x");
                assert_eq!(pair[1].kind(), "reference");
                assert_eq!(pair[1].value().value(), "&");
                assert!(pair[0].event() < pair[1].event());
            }
            assert_eq!(
                m.texts().iter().filter(|t| t.kind() == "reference").count(),
                2049
            );
        }
        let bytes = canonicalize_jats_unit_manifest(m, &f.profile, &f.budget()).unwrap();
        let decoded = decode_jats_unit_manifest(bytes.as_slice(), &f.profile, &f.budget()).unwrap();
        assert_eq!(&decoded, m);
        assert_eq!(
            verify_jats_unit_manifest(&decoded, &f.source(), &f.profile, &f.budget())
                .unwrap()
                .manifest(),
            m
        );
    }
}
#[test]
fn measured_producer_and_verifier_lower_boundaries() {
    let flag = AtomicBool::new(false);
    let f = Fixture::new(&specimen(), &flag);
    let set = f.set();
    let verified =
        verify_jats_unit_manifest(set.manifest(), &f.source(), &f.profile, &f.budget()).unwrap();
    for (verify, usage) in [(false, set.usage()), (true, verified.usage())] {
        for dimension in 0..3 {
            for below in [false, true] {
                let mut b = f.budget();
                let used = match dimension {
                    0 => usage.work,
                    1 => usage.canonical_bytes,
                    _ => usage.owned_bytes,
                } - usize::from(below);
                match dimension {
                    0 => b.limits.work_steps = used,
                    1 => b.limits.canonical_bytes = used,
                    _ => b.limits.owned_bytes = used,
                };
                let result = if verify {
                    verify_jats_unit_manifest(set.manifest(), &f.source(), &f.profile, &b)
                } else {
                    unitize_jats(&f.source(), &f.profile, &b)
                };
                assert_eq!(result.is_ok(), !below);
            }
        }
    }
    let bytes = canonicalize_jats_unit_manifest(set.manifest(), &f.profile, &f.budget()).unwrap();
    let mut b = f.budget();
    b.limits.encoded_bytes = bytes.len();
    assert!(unitize_jats(&f.source(), &f.profile, &b).is_ok());
    assert!(verify_jats_unit_manifest(set.manifest(), &f.source(), &f.profile, &b).is_ok());
    b.limits.encoded_bytes -= 1;
    assert_eq!(
        unitize_jats(&f.source(), &f.profile, &b).unwrap_err(),
        Error::CanonicalLimit
    );
    assert_eq!(
        verify_jats_unit_manifest(set.manifest(), &f.source(), &f.profile, &b).unwrap_err(),
        Error::CanonicalLimit
    );
}
#[test]
fn preflight_binding_profile_counts_values_and_independent_raw_work() {
    let flag = AtomicBool::new(false);
    let f = Fixture::new(&specimen(), &flag);
    let before = f.port.reads.load(Ordering::Relaxed);
    let mut b = f.budget();
    b.limits.nodes = f.product.elements().len() - 1;
    assert!(unitize_jats(&f.source(), &f.profile, &b).is_err());
    let mut b = f.budget();
    b.limits.metadata_bytes_each = 1;
    assert_eq!(
        unitize_jats(&f.source(), &f.profile, &b).unwrap_err(),
        Error::InvalidStructure
    );
    let mut b = f.budget();
    b.limits.work_steps = RAW_BUDGET.max_steps as usize + REPLAY.parser.work_steps - 1;
    assert_eq!(
        unitize_jats(&f.source(), &f.profile, &b).unwrap_err(),
        Error::WorkLimit
    );
    assert_eq!(f.port.reads.load(Ordering::Relaxed), before);
    assert!(
        JatsSourceBinding::new(f.binding.raw().clone(), f.binding.raw().representation_id).is_err()
    );
    let mismatch = JatsUnitizerProfile::new(
        f.unitizer.id().clone(),
        Blake3Digest32::from_stored_bytes([8; 32]),
        MAX_JATS_UNITIZER_LIMITS,
    )
    .unwrap();
    assert_eq!(
        unitize_jats(&f.source(), &mismatch, &f.budget()).unwrap_err(),
        Error::ProfileMismatch
    );
    assert_eq!(f.port.reads.load(Ordering::Relaxed), before);
}
#[test]
fn original_entry_live_and_terminal_cancel_deadline() {
    let flag = AtomicBool::new(false);
    let f = Fixture::new(&specimen(), &flag);
    let set = f.set();
    let before = f.port.reads.load(Ordering::Relaxed);
    flag.store(true, Ordering::Release);
    assert_eq!(
        unitize_jats(&f.source(), &f.profile, &f.budget()).unwrap_err(),
        Error::Cancelled
    );
    flag.store(false, Ordering::Release);
    let mut b = f.budget();
    b.deadline = Instant::now();
    assert_eq!(
        unitize_jats(&f.source(), &f.profile, &b).unwrap_err(),
        Error::DeadlineExceeded
    );
    assert_eq!(f.port.reads.load(Ordering::Relaxed), before);
    f.port.action.store(1, Ordering::Relaxed);
    assert_eq!(
        unitize_jats(&f.source(), &f.profile, &f.budget()).unwrap_err(),
        Error::Cancelled
    );
    assert_eq!(f.port.reads.load(Ordering::Relaxed), before + 1);
    f.port.action.store(0, Ordering::Relaxed);
    flag.store(false, Ordering::Release);
    for verify in [false, true] {
        for action in [1, 2] {
            let mut b = f.budget();
            if action == 2 {
                b.deadline = Instant::now() + Duration::from_secs(1);
            }
            let before = f.port.reads.load(Ordering::Relaxed);
            HOOK.with(|h| h.set((verify, action, 0)));
            let result = if verify {
                verify_jats_unit_manifest(set.manifest(), &f.source(), &f.profile, &b)
            } else {
                unitize_jats(&f.source(), &f.profile, &b)
            };
            assert_eq!(HOOK.with(Cell::get).2, 1);
            HOOK.with(|h| h.set((false, 0, 0)));
            assert_eq!(
                result.unwrap_err(),
                if action == 1 {
                    Error::Cancelled
                } else {
                    Error::DeadlineExceeded
                }
            );
            assert_eq!(f.port.reads.load(Ordering::Relaxed), before + 1);
            flag.store(false, Ordering::Release);
        }
    }
}
#[test]
fn deterministic_private_output_and_collision_guard() {
    let flag = AtomicBool::new(false);
    let f = Fixture::new(&specimen(), &flag);
    let a = f.set();
    let b = f.set();
    assert_eq!(a.manifest(), b.manifest());
    for output in [
        format!("{:?}", a),
        format!("{:?}", a.manifest().attributes()),
        format!("{:?}", a.manifest().texts()),
        Error::InvalidStructure.to_string(),
    ] {
        for secret in ["private-id", "urn:private-destination", "Citation", "Ж"] {
            assert!(!output.contains(secret));
        }
    }
    let mut ids = std::collections::BTreeMap::new();
    let digest = Blake3Digest32::from_stored_bytes([1; 32]);
    let id = super::build::truncated(digest);
    super::build::record(&mut ids, id, digest).unwrap();
    let mut collision = digest.as_bytes().to_owned();
    collision[31] ^= 1;
    let collision = Blake3Digest32::from_stored_bytes(collision);
    assert_eq!(id, super::build::truncated(collision));
    assert_eq!(
        super::build::record(&mut ids, id, collision),
        Err(Error::IdentityCollision)
    );
}

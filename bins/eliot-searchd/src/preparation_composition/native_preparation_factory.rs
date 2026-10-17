//! Native raw-code preparation through the original Root, C02, C03 and C07 owners.
//!
//! The module is included under `direct_registry` so it can consume the sealed
//! `RetentionSourceInput` and its existing member/C07 callers. It owns no path,
//! database, credential, query or provider client.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::preparation_composition::artifact_payloads::RawCodeArtifactProducerInput;
use search_contracts::indexed::{
    CollectionSchema, MAX_STORED_VECTOR_VALUES, NamedVectorRequirement, VectorName,
    VectorRequirement,
};
use search_contracts::{
    self as c, AssuranceClass, Blake3Digest32, BoundedList, CanonicalDigestDomain, CanonicalValue,
    DigestInputLimit, Materialization, MaterializationId, NonZeroRevision, OpaqueId, ProfileId,
    ProjectionMembership, ReceiptRef, Representation, RepresentationId, SourceId,
    SourceMembershipId, SourceRevisionRef, UnitKind,
};
use search_control_redb::{
    CommitRecoveryDecision, ControlSnapshotPublisher, MigrationBootstrapWork as Work, MutationId,
    PersistentControlJournal, PreparationInputMetadataV1, PreparationInputReadbackV1,
    PreparationInputRecordV1, PublicationProjectionSnapshot,
};
use search_lexical::{
    AcceptedCodeIdentifiersProfile, CodeIdentifierLimits, CodeIdentifiersProfile, SparseError,
    SparseLimits, qualify_code_identifiers_profile,
};
use search_materializer::api::{
    AcceptedProfiles, AssuranceCeiling, CancellationToken, DEFAULT_MATERIALIZATION_BUDGET,
    MaterializationContext, MaterializationError, MaterializationProduct, MaterializationRequest,
    RevisionReadPort, SourceEncoding, SourceKind, StoredRevisionBytes,
    ValidatedMaterializationRequest, ValidatedMaterializerProfile, baseline_profile_descriptor,
    materialize_text_or_code, validate_materialization_request, validate_materializer_profile,
    verify_materialization,
};
use search_point_identity::s11::PointIdentityLimits;
use search_projection_planner::{
    LineageObservation, PlanningScope, ProjectionBudget, ProjectionMembershipBinding,
    RawCodeProjectionProfileSetDescriptor,
};
use search_revision_store::{DomainBinding, residency_canonical_bytes};
use search_runtime_owner::OwnerError;
use search_source_registry::{
    ProjectionMembershipBindRequest, RegistryChange, RegistryOperation,
    api::{ProjectionMembershipReceipt, bind_projection_membership, read_projection_membership},
    error::{RegistryControlPort, RegistryError},
    recovery::{RegistryBatchLookup, RegistryStateRecord},
};
use search_unitizer::{
    DEFAULT_UNITIZATION_LIMITS, UnitizationBudget, UnitizationLimits, V3AnchorPolicy,
    V3AttachmentPolicy, V3EmptyPolicy, V3OmissionPolicy, V3OverlapPolicy, V3RepresentationKind,
    V3SourceBinding, V3UnitizerProfileDescriptor, ValidatedV3UnitizerProfile, VerifiedUnitSet,
    build_unit_manifest, canonicalize_unit_manifest, prepare_unit_set_input,
    validate_v3_unitizer_profile, verify_unit_manifest,
};

use super::retention_control::NativeRetentionControl;
use super::retention_source::RetentionSourceInput;
use super::{command::original_port, retention};
use crate::config_composition::EffectiveDaemonConfig;
use crate::registry_control::SourceIdentityIdsV2;
use crate::registry_control::native_member_setup::NativeMemberProfiles;
use c::NativeAnchor;

const RAW_CODE_SCHEMA_PROFILE_ID: &str = "raw-code-projection-schema-v1";
const RAW_CODE_VECTOR_NAME: &str = "lex_code_v1";
const RAW_CODE_INDEX_CEILING: u32 = 1_048_576;
const C09_FIXTURE_SOURCE: &[u8] = b"fn main() {}\n";
const C09_FIXTURE_DEFINITION: &[u8] =
    b"native-raw-code-unitizer-v3:fixture=fn main() {}\n;utf8-exact;units=1;span=0..input-len;omitted=0;exact-byte-anchor;verified-manifest";

/// Owner-produced C08/C09/C11 profiles. Construction executes a frozen C08/C09
/// raw-code fixture and C11's own complete runtime qualification before any
/// membership or collection identity is selected.
struct QualifiedNativeProfiles {
    materializer: ValidatedMaterializerProfile,
    unitizer: ValidatedV3UnitizerProfile,
    lexical: AcceptedCodeIdentifiersProfile,
}

impl QualifiedNativeProfiles {
    fn qualify<F>(
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &dyn Fn(Work) -> Result<(), OwnerError>,
        checkpoint: &mut F,
    ) -> Result<Self, &'static str>
    where
        F: FnMut() -> Result<(), SparseError>,
    {
        check_original(deadline, cancelled, work)?;
        let materializer =
            validate_materializer_profile(&baseline_profile_descriptor("257-baseline-utf8", 1))
                .map_err(|_| "NATIVE_PREPARATION_C08_PROFILE_REFUSED")?;
        let unitizer = validate_v3_unitizer_profile(&raw_code_unitizer_descriptor()?)
            .map_err(|_| "NATIVE_PREPARATION_C09_PROFILE_REFUSED")?;
        verify_c08_c09_raw_code_fixture(&materializer, &unitizer, deadline, cancelled, work)?;
        let lexical = qualify_code_identifiers_profile(
            CodeIdentifiersProfile::new().map_err(|_| "NATIVE_PREPARATION_C11_PROFILE_REFUSED")?,
            CodeIdentifierLimits::HARD,
            SparseLimits::BASELINE,
            deadline,
            checkpoint,
        )
        .map_err(|_| "NATIVE_PREPARATION_C11_PROFILE_QUALIFICATION_REFUSED")?;
        check_original(deadline, cancelled, work)?;
        Ok(Self {
            materializer,
            unitizer,
            lexical,
        })
    }

    const fn member_profiles(&self) -> NativeMemberProfiles<'_> {
        NativeMemberProfiles {
            materializer: &self.materializer,
            unitizer: &self.unitizer,
            lexical: &self.lexical,
        }
    }
}

fn check_original(
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &dyn Fn(Work) -> Result<(), OwnerError>,
) -> Result<(), &'static str> {
    if cancelled.load(std::sync::atomic::Ordering::Acquire) || Instant::now() >= deadline {
        return Err("NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED");
    }
    work(Work::Checkpoint).map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
    if cancelled.load(std::sync::atomic::Ordering::Acquire) || Instant::now() >= deadline {
        return Err("NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED");
    }
    Ok(())
}

fn raw_code_unitizer_descriptor() -> Result<V3UnitizerProfileDescriptor, &'static str> {
    let domain = CanonicalDigestDomain::parse("eliot/raw/unitizer-fixture/v3")
        .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_INVALID")?;
    let limit =
        DigestInputLimit::new(4096).map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_INVALID")?;
    let fixture_digest = c::blake3_raw(&domain, C09_FIXTURE_DEFINITION, limit)
        .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_INVALID")?;
    let limits = UnitizationLimits {
        max_input_bytes: DEFAULT_UNITIZATION_LIMITS
            .max_input_bytes
            .min(c::MAX_CANONICAL_BYTES - 128),
        max_units: 4096,
        ..DEFAULT_UNITIZATION_LIMITS
    };
    Ok(V3UnitizerProfileDescriptor {
        profile_name: ProfileId::new("baseline-unitizer-v3")
            .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_INVALID")?,
        profile_revision: 1,
        representation_kind: V3RepresentationKind::Code,
        unit_kind: UnitKind::Section,
        boundary_revision: 1,
        overlap_policy: V3OverlapPolicy::None,
        anchor_policy: V3AnchorPolicy::ExactTextBytes,
        attachment_policy: V3AttachmentPolicy::Absent,
        omission_policy: V3OmissionPolicy::Forbidden,
        empty_policy: V3EmptyPolicy::Reject,
        limits,
        min_unit_bytes: 1,
        max_unit_scalars: limits.max_unit_bytes,
        max_unit_lines: limits.max_unit_bytes,
        max_anchor_depth: 1,
        max_manifest_bytes: c::MAX_CANONICAL_BYTES - 128,
        max_steps: 32_000_000,
        fixture_digest,
    })
}

struct FrozenRevisionPort<'a> {
    source: &'a OpaqueId,
    revision: NonZeroRevision,
    residency: &'a OpaqueId,
    bytes: &'a [u8],
    digest: Blake3Digest32,
    cancelled: &'a AtomicBool,
    deadline: Instant,
    work: &'a dyn Fn(Work) -> Result<(), OwnerError>,
}

impl RevisionReadPort for FrozenRevisionPort<'_> {
    fn read_exact(
        &self,
        source: &OpaqueId,
        revision: NonZeroRevision,
        byte_count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError> {
        if source != self.source
            || revision != self.revision
            || byte_count != u64::try_from(self.bytes.len()).unwrap_or(u64::MAX)
            || self.cancelled.load(std::sync::atomic::Ordering::Acquire)
            || Instant::now() >= self.deadline
        {
            return Err(MaterializationError::Cancelled);
        }
        let size = self
            .bytes
            .len()
            .checked_mul(2)
            .ok_or(MaterializationError::BudgetExhausted)?;
        (self.work)(Work::Allocate(size)).map_err(|_| MaterializationError::Cancelled)?;
        (self.work)(Work::Read(self.bytes.len())).map_err(|_| MaterializationError::Cancelled)?;
        (self.work)(Work::Process(size)).map_err(|_| MaterializationError::Cancelled)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.bytes.len())
            .map_err(|_| MaterializationError::BudgetExhausted)?;
        bytes.extend_from_slice(self.bytes);
        Ok(StoredRevisionBytes::new(
            bytes,
            self.digest,
            self.residency.clone(),
        ))
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the C08/C09 fixture execution and exact output checks in one owner path"
)]
fn verify_c08_c09_raw_code_fixture(
    materializer: &ValidatedMaterializerProfile,
    unitizer: &ValidatedV3UnitizerProfile,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &dyn Fn(Work) -> Result<(), OwnerError>,
) -> Result<(), &'static str> {
    check_original(deadline, cancelled, work)?;
    let fixture_source = SourceId::from_bytes([0x51; 16]);
    let fixture_namespace = c::SourceNamespaceId::from_bytes([0x52; 16]);
    let fixture_revision_id = c::SourceRevisionId::from_bytes([0x53; 16]);
    let fixture_materialization_id = MaterializationId::from_bytes([0x54; 16]);
    let fixture_representation_id = RepresentationId::from_bytes([0x55; 16]);
    let source_id = OpaqueId::new(fixture_source.to_string())
        .map_err(|_| "NATIVE_PREPARATION_C08_FIXTURE_INVALID")?;
    let revision = NonZeroRevision::new(1).map_err(|_| "NATIVE_PREPARATION_C08_FIXTURE_INVALID")?;
    let residency = OpaqueId::new("residency:raw-code-profile-fixture")
        .map_err(|_| "NATIVE_PREPARATION_C08_FIXTURE_INVALID")?;
    let operation = OpaqueId::new("operation:raw-code-profile-fixture")
        .map_err(|_| "NATIVE_PREPARATION_C08_FIXTURE_INVALID")?;
    let input_digest = Blake3Digest32::from_bytes(*blake3::hash(C09_FIXTURE_SOURCE).as_bytes());
    let raw_request = MaterializationRequest {
        source_id: source_id.clone(),
        revision,
        residency: residency.clone(),
        content_digest: input_digest,
        byte_count: u64::try_from(C09_FIXTURE_SOURCE.len())
            .map_err(|_| "NATIVE_PREPARATION_C08_FIXTURE_INVALID")?,
        declared_kind: SourceKind::Code,
        declared_encoding: SourceEncoding::Utf8,
        profile_id: materializer.id(),
        operation_id: operation,
        from_unsaved_bytes: false,
        unsaved_snapshot_receipt: None,
    };
    let request = validate_materialization_request(
        &raw_request,
        &AcceptedProfiles::new(vec![materializer.clone()]),
        &search_materializer::api::DEFAULT_MATERIALIZATION_BUDGET,
    )
    .map_err(|_| "NATIVE_PREPARATION_C08_FIXTURE_REFUSED")?;
    let port = FrozenRevisionPort {
        source: &source_id,
        revision,
        residency: &residency,
        bytes: C09_FIXTURE_SOURCE,
        digest: input_digest,
        cancelled,
        deadline,
        work,
    };
    let context = MaterializationContext {
        profile: materializer.clone(),
        budget: search_materializer::api::DEFAULT_MATERIALIZATION_BUDGET,
        cancel: CancellationToken::new(cancelled),
    };
    let product = materialize_text_or_code(&request, &port, &context)
        .map_err(|_| "NATIVE_PREPARATION_C08_FIXTURE_REFUSED")?;
    verify_materialization(&product, &request, materializer)
        .map_err(|_| "NATIVE_PREPARATION_C08_FIXTURE_REFUSED")?;
    if product.canonical_text().as_bytes() != C09_FIXTURE_SOURCE
        || product.input_digest() != input_digest
        || product.canonical_digest() != input_digest
        || product.assurance().ceiling() != AssuranceCeiling::ExactBytes
        || !product.maps().loss_map().records().is_empty()
    {
        return Err("NATIVE_PREPARATION_C08_EXPECTED_OUTPUT_MISMATCH");
    }
    check_original(deadline, cancelled, work)?;

    let binding = V3SourceBinding {
        source_namespace_id: fixture_namespace,
        source_id: fixture_source,
        source_revision_id: fixture_revision_id,
        representation_id: fixture_representation_id,
        materialization_id: fixture_materialization_id,
    };
    let budget = UnitizationBudget {
        max_steps: unitizer.descriptor().max_steps,
        max_encoded_bytes: unitizer.descriptor().max_manifest_bytes,
        deadline,
        cancelled,
    };
    let input = prepare_unit_set_input(
        binding.clone(),
        &product,
        &request,
        materializer,
        unitizer,
        &budget,
    )
    .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_REFUSED")?;
    let set = build_unit_manifest(&input, unitizer, &budget)
        .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_REFUSED")?;
    let verified = verify_unit_manifest(set.manifest(), &input, unitizer, &budget)
        .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_REFUSED")?;
    let canonical = canonicalize_unit_manifest(set.manifest())
        .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_REFUSED")?;
    if verified != set
        || set.manifest().provenance().binding() != &binding
        || set.manifest().emitted_bytes()
            != u64::try_from(C09_FIXTURE_SOURCE.len())
                .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_INVALID")?
        || set.manifest().omitted_bytes() != 0
        || set.units().len() != 1
        || set.units()[0].source_start() != 0
        || set.units()[0].source_end()
            != u64::try_from(C09_FIXTURE_SOURCE.len())
                .map_err(|_| "NATIVE_PREPARATION_C09_FIXTURE_INVALID")?
        || set.units()[0].occurrence().representation_id != fixture_representation_id
        || !matches!(
            &set.units()[0].occurrence().native_anchor,
            NativeAnchor::TextBytes(anchor)
                if anchor.byte_start_0 == 0
                    && anchor.byte_end_exclusive_0
                        == u64::try_from(C09_FIXTURE_SOURCE.len()).unwrap_or(u64::MAX)
        )
        || canonical.as_slice().is_empty()
    {
        return Err("NATIVE_PREPARATION_C09_EXPECTED_OUTPUT_MISMATCH");
    }
    check_original(deadline, cancelled, work)
}

fn read_c02_state(
    input: &RetentionSourceInput<'_, '_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
) -> Result<
    (
        Option<PreparationInputReadbackV1>,
        PublicationProjectionSnapshot,
    ),
    &'static str,
> {
    let identity = journal.identity();
    let work = |unit| input.native.work(unit);
    let control = journal
        .bind_control_port::<super::super::read_child::fresh_root::FreshRootCancellation>(publisher)
        .map_err(|_| "NATIVE_PREPARATION_C02_BIND_REFUSED")?;
    let preparation = control
        .read_preparation_input_with_original_context(
            &input.command.operation,
            input.deadline,
            &work,
        )
        .map_err(|_| "NATIVE_PREPARATION_C02_ORIGINAL_READ_REFUSED")?;
    let snapshot = control
        .read_publication_projection_snapshot_with_original_context(input.deadline, &work)
        .map_err(|_| "NATIVE_PREPARATION_C02_PROJECTION_SNAPSHOT_REFUSED")?
        .ok_or("NATIVE_PREPARATION_C02_VISIBILITY_MISSING")?;
    if snapshot.identity() != identity {
        return Err("NATIVE_PREPARATION_C02_IDENTITY_MISMATCH");
    }
    Ok((preparation, snapshot))
}

fn commit_c02_input(
    input: &RetentionSourceInput<'_, '_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    record: &PreparationInputRecordV1,
) -> Result<(), &'static str> {
    let work = |unit| input.native.work(unit);
    let mut control = journal
        .bind_control_port::<super::super::read_child::fresh_root::FreshRootCancellation>(publisher)
        .map_err(|_| "NATIVE_PREPARATION_C02_BIND_REFUSED")?;
    match control.transact_preparation_input_with_original_context(record, input.deadline, &work) {
        Ok(_) => {}
        Err(failure)
            if matches!(
                failure.control,
                search_control_redb::ControlError::CommitOutcomeUnknown
            ) =>
        {
            match control.recover_preparation_input_with_original_context(
                record,
                input.deadline,
                &work,
            ) {
                Ok(CommitRecoveryDecision::Committed(_)) => {}
                Ok(CommitRecoveryDecision::NotCommittedRetrySameOperation) => {
                    return Err("NATIVE_PREPARATION_C02_NOT_COMMITTED");
                }
                _ => return Err("NATIVE_PREPARATION_C02_RECOVERY_UNRESOLVED"),
            }
        }
        Err(_) => return Err("NATIVE_PREPARATION_C02_COMMIT_REFUSED"),
    }
    let readback = control
        .read_preparation_input_with_original_context(
            record.original_operation_id(),
            input.deadline,
            &work,
        )
        .map_err(|_| "NATIVE_PREPARATION_C02_READBACK_UNKNOWN")?
        .ok_or("NATIVE_PREPARATION_C02_READBACK_MISSING")?;
    if readback.record() != record {
        return Err("NATIVE_PREPARATION_C02_READBACK_MISMATCH");
    }
    Ok(())
}

fn canonical_text(value: &str) -> Result<CanonicalValue, &'static str> {
    c::CanonicalText::new(value.to_owned())
        .map(CanonicalValue::Text)
        .map_err(|_| "NATIVE_PREPARATION_CANONICAL_VALUE_INVALID")
}

fn canonical_bytes(value: &[u8]) -> Result<CanonicalValue, &'static str> {
    c::BoundedBytes::new(value.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| "NATIVE_PREPARATION_CANONICAL_VALUE_INVALID")
}

fn canonical_array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, &'static str> {
    BoundedList::new(values)
        .map(CanonicalValue::Array)
        .map_err(|_| "NATIVE_PREPARATION_CANONICAL_VALUE_INVALID")
}

fn encode_canonical(value: &CanonicalValue) -> Result<Vec<u8>, &'static str> {
    c::to_canonical_cbor(value)
        .map(|encoded| encoded.as_slice().to_vec())
        .map_err(|_| "NATIVE_PREPARATION_CANONICAL_VALUE_INVALID")
}

fn hex_digest(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(*byte >> 4)]));
        output.push(char::from(HEX[usize::from(*byte & 0x0f)]));
    }
    output
}

fn projection_operation(
    original: &OpaqueId,
    membership: SourceMembershipId,
) -> Result<(OpaqueId, ReceiptRef), &'static str> {
    let value = canonical_array(vec![
        canonical_text("native-projection-bind-operation/v1")?,
        canonical_text(original.as_str())?,
        canonical_bytes(membership.as_bytes())?,
    ])?;
    let bytes = encode_canonical(&value)?;
    let domain = CanonicalDigestDomain::parse("eliot/raw/native-projection-bind-operation/v1")
        .map_err(|_| "NATIVE_PREPARATION_PROJECTION_OPERATION_INVALID")?;
    let digest = c::blake3_raw(
        &domain,
        &bytes,
        DigestInputLimit::new(4096)
            .map_err(|_| "NATIVE_PREPARATION_PROJECTION_OPERATION_INVALID")?,
    )
    .map_err(|_| "NATIVE_PREPARATION_PROJECTION_OPERATION_INVALID")?;
    let suffix = hex_digest(digest.as_bytes());
    Ok((
        OpaqueId::new(format!("native-projection-bind:{suffix}"))
            .map_err(|_| "NATIVE_PREPARATION_PROJECTION_OPERATION_INVALID")?,
        ReceiptRef::new(format!("native-projection-receipt:{suffix}"))
            .map_err(|_| "NATIVE_PREPARATION_PROJECTION_OPERATION_INVALID")?,
    ))
}

fn raw_code_profile_set(
    profiles: &QualifiedNativeProfiles,
    vector: &NamedVectorRequirement,
) -> Result<RawCodeProjectionProfileSetDescriptor, &'static str> {
    let code = profiles.lexical.profile();
    RawCodeProjectionProfileSetDescriptor::new(
        code.id().clone(),
        code.revision(),
        code.fingerprint(),
        code.artifact_set_identity(),
        code.compatibility_fixture_identity(),
        code.collision_fixture_identity(),
        vector.clone(),
        Blake3Digest32::from_bytes(*profiles.materializer.id().as_bytes()),
        profiles.unitizer.id().clone(),
    )
    .map_err(|_| "NATIVE_PREPARATION_C13_PROFILE_SET_REFUSED")
}

fn partition_semantics(
    workspace_id: &[u8; 16],
    corpus_id: &[u8; 16],
    input: &RetentionSourceInput<'_, '_, '_>,
    domains: &[DomainBinding; 6],
    profiles: &QualifiedNativeProfiles,
    profile_set: &RawCodeProjectionProfileSetDescriptor,
    vector: &NamedVectorRequirement,
) -> Result<search_source_registry::ProjectionPartitionSemantics, &'static str> {
    let permanent = input
        .namespace
        .permanent_metadata()
        .ok_or("NATIVE_PREPARATION_NAMESPACE_POLICY_MISSING")?;
    let access = canonical_array(vec![
        canonical_text("native-access-partition/raw-code/v1")?,
        canonical_bytes(workspace_id)?,
        canonical_bytes(corpus_id)?,
        canonical_bytes(domains[0].canonical_body())?,
        canonical_bytes(domains[1].canonical_body())?,
        canonical_bytes(domains[2].canonical_body())?,
        canonical_bytes(input.security.policy_body())?,
        canonical_bytes(permanent.policy().encoded_inputs())?,
    ])?;

    let code = profiles.lexical.profile();
    let scoring = canonical_array(vec![
        canonical_text("native-scoring-partition/raw-code/v1")?,
        canonical_bytes(corpus_id)?,
        canonical_text(profile_set.profile_set_id().as_str())?,
        canonical_text(code.id().as_str())?,
        CanonicalValue::U64(code.revision().get()),
        canonical_bytes(code.fingerprint().as_bytes())?,
        canonical_bytes(code.artifact_set_identity().as_bytes())?,
        canonical_bytes(code.compatibility_fixture_identity().as_bytes())?,
        canonical_bytes(code.collision_fixture_identity().as_bytes())?,
        canonical_text(profiles.materializer.name())?,
        CanonicalValue::U64(profiles.materializer.revision()),
        canonical_bytes(profiles.materializer.golden_fixture_digest().as_bytes())?,
        canonical_text(profiles.unitizer.id().as_str())?,
        CanonicalValue::U64(profiles.unitizer.revision()),
        canonical_bytes(profiles.unitizer.descriptor().fixture_digest.as_bytes())?,
        vector
            .to_canonical_value()
            .map_err(|_| "NATIVE_PREPARATION_C13_PROFILE_SET_REFUSED")?,
    ])?;
    search_source_registry::ProjectionPartitionSemantics::new(
        encode_canonical(&access)?,
        encode_canonical(&scoring)?,
    )
    .map_err(|_| "NATIVE_PREPARATION_PARTITION_SEMANTICS_INVALID")
}

const fn assurance_class(value: AssuranceCeiling) -> AssuranceClass {
    match value {
        AssuranceCeiling::ExactBytes => AssuranceClass::ExactBytes,
        AssuranceCeiling::ExactTranscoded => AssuranceClass::MappedText,
        AssuranceCeiling::NormalizedWithRecordedLoss => AssuranceClass::LossyText,
    }
}

fn projection_mutation_digest(
    request: &ProjectionMembershipBindRequest,
    registry_revision_after: u64,
    receipt: &ReceiptRef,
) -> Result<Blake3Digest32, &'static str> {
    let record = search_source_registry::ProjectionMembershipRecord::from_observed_parts(
        request.clone(),
        registry_revision_after,
        receipt.clone(),
    )
    .map_err(|_| "NATIVE_PREPARATION_C03_REQUEST_INVALID")?;
    let state = RegistryStateRecord::ProjectionMembership(record);
    let encoded = state
        .encode_with_profile("eliot.registry-state.v7")
        .map_err(|_| "NATIVE_PREPARATION_C03_REQUEST_INVALID")?;
    let domain = CanonicalDigestDomain::parse("eliot/raw/root-projection-operation/v1")
        .map_err(|_| "NATIVE_PREPARATION_C03_REQUEST_INVALID")?;
    c::blake3_raw(
        &domain,
        &encoded,
        DigestInputLimit::new(c::MAX_CANONICAL_BYTES)
            .map_err(|_| "NATIVE_PREPARATION_C03_REQUEST_INVALID")?,
    )
    .map_err(|_| "NATIVE_PREPARATION_C03_REQUEST_INVALID")
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep original-operation replay, first-bind and readback ordering under one context"
)]
fn bind_projection_exact(
    input: &RetentionSourceInput<'_, '_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    request: ProjectionMembershipBindRequest,
    operation_id: OpaqueId,
    receipt: ReceiptRef,
) -> Result<ProjectionMembershipReceipt, &'static str> {
    input
        .native
        .source_full_checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
    input
        .security
        .checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ACCESS_REFUSED")?;
    if let Some(current_source) = input.current_source {
        current_source().map_err(|_| "NATIVE_PREPARATION_SOURCE_FENCE_REFUSED")?;
    }
    let work = |unit| input.native.work(unit);
    let mut port = original_port(journal, publisher, input.deadline, &work)
        .map_err(|_| "NATIVE_PREPARATION_C03_BIND_REFUSED")?;
    let mut registry = port
        .restore(input.native.context())
        .map_err(|_| "NATIVE_PREPARATION_C03_RESTORE_REFUSED")?
        .into_registry();
    let source_membership_id = request.binding.source_membership_id;
    let lookup = port
        .load_batch(&operation_id, input.native.context())
        .map_err(|_| "NATIVE_PREPARATION_C03_OPERATION_READ_REFUSED")?;
    let (expected_revision, operation, effective_request, effective_receipt) = match lookup {
        RegistryBatchLookup::Committed(observed) => {
            observed
                .verify_operation()
                .map_err(|_| "NATIVE_PREPARATION_C03_OPERATION_CONFLICT")?;
            let batch = observed
                .record()
                .replay_batch()
                .ok_or("NATIVE_PREPARATION_C03_ORIGINAL_MISSING")?;
            let [
                RegistryChange::BindProjectionMembership {
                    request: original_request,
                    receipt: original_receipt,
                },
            ] = batch.changes.as_slice()
            else {
                return Err("NATIVE_PREPARATION_C03_ORIGINAL_CONFLICT");
            };
            if batch.operation.operation_id() != &operation_id
                || original_request != &request
                || original_receipt != &receipt
            {
                return Err("NATIVE_PREPARATION_C03_ORIGINAL_CONFLICT");
            }
            (
                batch.expected_registry_revision,
                batch.operation.clone(),
                original_request.clone(),
                original_receipt.clone(),
            )
        }
        RegistryBatchLookup::Absent { revision } => {
            if revision != registry.revision()
                || registry
                    .projection_membership_record(source_membership_id)
                    .is_ok()
            {
                return Err("NATIVE_PREPARATION_C03_FIRST_BIND_CONFLICT");
            }
            let after = registry
                .revision()
                .checked_add(1)
                .ok_or("NATIVE_PREPARATION_C03_REVISION_EXHAUSTED")?;
            let digest = projection_mutation_digest(&request, after, &receipt)?;
            (
                registry.revision(),
                RegistryOperation::new(operation_id, digest),
                request,
                receipt,
            )
        }
    };
    let bound = bind_projection_membership(
        &mut registry,
        effective_request,
        expected_revision,
        operation,
        &effective_receipt,
        &mut port,
        input.native.context(),
    )
    .map_err(|_| "NATIVE_PREPARATION_C03_BIND_REFUSED")?;
    let current = read_projection_membership(&registry, source_membership_id, registry.revision())
        .map_err(|_| "NATIVE_PREPARATION_C03_READBACK_REFUSED")?;
    if current.binding() != &bound.binding {
        return Err("NATIVE_PREPARATION_C03_READBACK_MISMATCH");
    }
    drop(port);
    if let Some(current_source) = input.current_source {
        current_source().map_err(|_| "NATIVE_PREPARATION_SOURCE_FENCE_REFUSED")?;
    }
    input
        .security
        .checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ACCESS_REFUSED")?;
    input
        .native
        .source_full_checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
    Ok(bound)
}

struct PreparedSourceOwners {
    product: MaterializationProduct,
    units: VerifiedUnitSet,
    request: ValidatedMaterializationRequest,
    materialization: Materialization,
    representation: Representation,
    residency_id: OpaqueId,
}

fn materialization_from_product(
    product: &MaterializationProduct,
    source_revision: &SourceRevisionRef,
    materialization_id: MaterializationId,
    residency: c::ObjectResidencyKeyDigest,
    materializer: &ValidatedMaterializerProfile,
) -> Result<Materialization, &'static str> {
    Ok(Materialization {
        materialization_id,
        source_revision_id: source_revision.revision_id,
        materializer_profile_id: ProfileId::new(materializer.name())
            .map_err(|_| "NATIVE_PREPARATION_C08_PROFILE_ID_INVALID")?,
        canonical_object_digest: product.canonical_digest(),
        object_residency_key_digest: residency,
        native_coordinate_map_digest: product.coordinate_digest(),
        loss_map_digest: product.loss_digest(),
        assurance_ceiling: assurance_class(product.assurance().ceiling()),
    })
}

fn representation_from_units(
    units: &VerifiedUnitSet,
    materialization_id: MaterializationId,
    representation_id: RepresentationId,
    unitizer: &ValidatedV3UnitizerProfile,
) -> Result<Representation, &'static str> {
    Ok(Representation {
        representation_id,
        materialization_id,
        unitizer_profile_id: unitizer.id().clone(),
        enrichment_profile_ids: BoundedList::new(Vec::new())
            .map_err(|_| "NATIVE_PREPARATION_REPRESENTATION_INVALID")?,
        unit_manifest_digest: units.manifest_digest(),
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "Verify materialization and unitization through their owners before C02 binds their identities"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Carry the original retained-source context and exact C07 stores without a second owner object"
)]
fn prepare_retained_source_owners(
    input: &RetentionSourceInput<'_, '_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    config: &EffectiveDaemonConfig,
    profiles: &QualifiedNativeProfiles,
    source_revision: &SourceRevisionRef,
    materialization_id: MaterializationId,
    representation_id: RepresentationId,
) -> Result<PreparedSourceOwners, &'static str> {
    let result = input
        .existing
        .result
        .as_ref()
        .ok_or("NATIVE_PREPARATION_RETAINED_SOURCE_REQUIRED")?;
    let profile = input
        .existing
        .profile
        .as_ref()
        .ok_or("NATIVE_PREPARATION_RETAINED_SOURCE_REQUIRED")?;
    let work = |unit| input.native.work(unit);
    let control = NativeRetentionControl::bind(
        journal,
        publisher,
        input.source,
        input.namespace,
        input.root,
        input.security,
        input.deadline,
        &work,
    )
    .map_err(|_| "NATIVE_PREPARATION_RETAINED_SOURCE_CONTROL_REFUSED")?;
    let (plaintext, metadata) = super::retained_reader::open_retained_revision(
        input.native,
        &control,
        source_revision,
        config,
    )
    .map_err(|_| "NATIVE_PREPARATION_RETAINED_SOURCE_READBACK_REFUSED")?;
    drop(control);
    if metadata.source().namespace_id != source_revision.source_namespace_id
        || metadata.source().source_id != source_revision.source_id
        || metadata.source().revision_id != source_revision.revision_id
        || metadata.content_digest() != source_revision.content_digest
        || metadata.content_bytes() != source_revision.byte_length
        || metadata.operation_id() != &profile.operation_id
    {
        return Err("NATIVE_PREPARATION_RETAINED_SOURCE_BINDING_MISMATCH");
    }
    let source_name = OpaqueId::new(source_revision.source_id.to_string())
        .map_err(|_| "NATIVE_PREPARATION_SOURCE_ID_INVALID")?;
    let revision = NonZeroRevision::new(result.revision.occurrence_sequence)
        .map_err(|_| "NATIVE_PREPARATION_SOURCE_SEQUENCE_INVALID")?;
    let request = MaterializationRequest {
        source_id: source_name.clone(),
        revision,
        residency: metadata.storage_object_id().clone(),
        content_digest: source_revision.content_digest,
        byte_count: source_revision.byte_length,
        declared_kind: SourceKind::Code,
        declared_encoding: SourceEncoding::Utf8,
        profile_id: profiles.materializer.id(),
        operation_id: input.command.operation.clone(),
        from_unsaved_bytes: false,
        unsaved_snapshot_receipt: None,
    };
    let materialization_budget = DEFAULT_MATERIALIZATION_BUDGET;
    let validated = validate_materialization_request(
        &request,
        &AcceptedProfiles::new(vec![profiles.materializer.clone()]),
        &materialization_budget,
    )
    .map_err(|_| "NATIVE_PREPARATION_C08_REQUEST_REFUSED")?;

    let cancelled = input.native.original_cancellation_flag();
    let body = plaintext.as_bytes();
    let maximum_input = usize::try_from(materialization_budget.max_input_bytes)
        .map_err(|_| "NATIVE_PREPARATION_RESOURCE_BOUNDS")?;
    if u64::try_from(body.len()).ok() != Some(source_revision.byte_length)
        || body.len() > maximum_input
    {
        return Err("NATIVE_PREPARATION_SOURCE_LENGTH_REFUSED");
    }
    let prepaid = body
        .len()
        .checked_mul(16)
        .and_then(|bytes| bytes.checked_add(64 * 1024))
        .ok_or("NATIVE_PREPARATION_RESOURCE_BOUNDS")?;
    for unit in [Work::Allocate(prepaid), Work::Process(prepaid)] {
        work(unit).map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
    }
    let port = FrozenRevisionPort {
        source: &source_name,
        revision,
        residency: metadata.storage_object_id(),
        bytes: body,
        digest: source_revision.content_digest,
        cancelled,
        deadline: input.deadline,
        work: &work,
    };
    let context = MaterializationContext {
        profile: profiles.materializer.clone(),
        budget: materialization_budget,
        cancel: CancellationToken::new(cancelled),
    };
    input
        .native
        .source_full_checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
    input
        .security
        .checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ACCESS_REFUSED")?;
    if let Some(current_source) = input.current_source {
        current_source().map_err(|_| "NATIVE_PREPARATION_SOURCE_FENCE_REFUSED")?;
    }
    let product = materialize_text_or_code(&validated, &port, &context)
        .map_err(|_| "NATIVE_PREPARATION_C08_SOURCE_REFUSED")?;
    verify_materialization(&product, &validated, &profiles.materializer)
        .map_err(|_| "NATIVE_PREPARATION_C08_SOURCE_VERIFY_REFUSED")?;
    let binding = V3SourceBinding {
        source_namespace_id: source_revision.source_namespace_id,
        source_id: source_revision.source_id,
        source_revision_id: source_revision.revision_id,
        representation_id,
        materialization_id,
    };
    let unitizer_budget = UnitizationBudget {
        max_steps: profiles.unitizer.descriptor().max_steps,
        max_encoded_bytes: profiles.unitizer.descriptor().max_manifest_bytes,
        deadline: input.deadline,
        cancelled,
    };
    let unitization_prepaid = body
        .len()
        .checked_mul(8)
        .and_then(|bytes| {
            profiles
                .unitizer
                .descriptor()
                .max_manifest_bytes
                .checked_mul(2)
                .and_then(|manifest| bytes.checked_add(manifest))
        })
        .ok_or("NATIVE_PREPARATION_RESOURCE_BOUNDS")?;
    for unit in [
        Work::Allocate(unitization_prepaid),
        Work::Process(unitization_prepaid),
    ] {
        work(unit).map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
    }
    let unit_input = prepare_unit_set_input(
        binding.clone(),
        &product,
        &validated,
        &profiles.materializer,
        &profiles.unitizer,
        &unitizer_budget,
    )
    .map_err(|_| "NATIVE_PREPARATION_C09_SOURCE_REFUSED")?;
    let unit_set = build_unit_manifest(&unit_input, &profiles.unitizer, &unitizer_budget)
        .map_err(|_| "NATIVE_PREPARATION_C09_SOURCE_REFUSED")?;
    let verified = verify_unit_manifest(
        unit_set.manifest(),
        &unit_input,
        &profiles.unitizer,
        &unitizer_budget,
    )
    .map_err(|_| "NATIVE_PREPARATION_C09_SOURCE_VERIFY_REFUSED")?;
    if verified != unit_set
        || unit_set.manifest().provenance().binding() != &binding
        || unit_set.representation().representation_id != representation_id
        || unit_set.representation().materialization_id != materialization_id
        || unit_set.representation().unit_manifest_digest != unit_set.manifest_digest()
    {
        return Err("NATIVE_PREPARATION_C09_SOURCE_BINDING_MISMATCH");
    }
    let materialization = materialization_from_product(
        &product,
        source_revision,
        materialization_id,
        result.revision.object_residency_key_digest,
        &profiles.materializer,
    )?;
    let representation = representation_from_units(
        &unit_set,
        materialization_id,
        representation_id,
        &profiles.unitizer,
    )?;
    if let Some(current_source) = input.current_source {
        current_source().map_err(|_| "NATIVE_PREPARATION_SOURCE_FENCE_REFUSED")?;
    }
    input
        .security
        .checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ACCESS_REFUSED")?;
    input
        .native
        .source_full_checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
    Ok(PreparedSourceOwners {
        product,
        units: unit_set,
        request: validated,
        materialization,
        representation,
        residency_id: metadata.storage_object_id().clone(),
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep owner qualification, C02 intent, C03 first-bind and C07 effects in their required order"
)]
pub(super) fn prepare_native_raw_code_artifacts(
    input: &RetentionSourceInput<'_, '_, '_>,
    journal: &mut PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    config: &EffectiveDaemonConfig,
) -> Result<
    crate::preparation_composition::native_artifact_store::NativeStoredArtifactSet,
    &'static str,
> {
    let original_result = input
        .existing
        .result
        .as_ref()
        .ok_or("NATIVE_PREPARATION_RETAINED_SOURCE_REQUIRED")?;
    let original_profile = input
        .existing
        .profile
        .as_ref()
        .ok_or("NATIVE_PREPARATION_RETAINED_SOURCE_REQUIRED")?;
    if original_result.operation_id != input.command.operation
        || original_profile.operation_id != input.command.operation
        || original_result.source_namespace_id != input.source.identity().source_namespace_id
        || original_result.source_id != input.source.identity().source_id
        || original_profile.source_namespace_id != original_result.source_namespace_id
        || original_profile.source_id != original_result.source_id
        || original_profile.revision_id != original_result.revision.revision_id
        || original_profile.occurrence_sequence != original_result.revision.occurrence_sequence
        || original_profile.content_digest != original_result.revision.content_digest
        || original_profile.byte_length != original_result.revision.byte_length
    {
        return Err("NATIVE_PREPARATION_RETAINED_SOURCE_BINDING_MISMATCH");
    }
    let source_revision = SourceRevisionRef {
        source_namespace_id: original_result.source_namespace_id,
        source_id: original_result.source_id,
        revision_id: original_result.revision.revision_id,
        content_digest: original_result.revision.content_digest,
        byte_length: original_result.revision.byte_length,
    };
    let work = |unit| input.native.work(unit);

    // The original C02 operation is looked up before profile setup can reach
    // any C03 or typed-identity allocator.
    let (retained_preparation, initial_snapshot) = read_c02_state(input, journal, publisher)?;
    if let Some(readback) = &retained_preparation {
        let metadata = readback.record().metadata();
        if readback.record().original_operation_id() != &input.command.operation
            || metadata.source_revision != source_revision
        {
            return Err("NATIVE_PREPARATION_C02_ORIGINAL_CONFLICT");
        }
        let source_membership_id = metadata.source_membership.source_membership_id;
        let port = original_port(journal, publisher, input.deadline, &work)
            .map_err(|_| "NATIVE_PREPARATION_C03_RESTORE_REFUSED")?;
        let registry = port
            .restore(input.native.context())
            .map_err(|_| "NATIVE_PREPARATION_C03_RESTORE_REFUSED")?
            .into_registry();
        let (member, source) = registry
            .active_source_membership_v2(source_membership_id)
            .map_err(|_| "NATIVE_PREPARATION_C02_MEMBER_MISSING")?;
        if member != &metadata.source_membership || source != input.source.identity() {
            return Err("NATIVE_PREPARATION_C02_MEMBER_CONFLICT");
        }
    }
    let initial_visibility = *initial_snapshot.visibility();
    let mut qualification_checkpoint = || {
        if input.native.source_full_checkpoint().is_err()
            || input.security.checkpoint().is_err()
            || input.current_source.is_some_and(|check| check().is_err())
        {
            Err(SparseError::Cancelled)
        } else {
            Ok(())
        }
    };
    let profiles = QualifiedNativeProfiles::qualify(
        input.deadline,
        input.native.original_cancellation_flag(),
        &work,
        &mut qualification_checkpoint,
    )?;
    input
        .native
        .source_full_checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
    input
        .security
        .checkpoint()
        .map_err(|_| "NATIVE_PREPARATION_ACCESS_REFUSED")?;

    let member_receipt = input
        .setup_raw_code_member(journal, publisher, config, &profiles.member_profiles())
        .map_err(|_| "NATIVE_PREPARATION_MEMBER_SETUP_REFUSED")?;
    if retained_preparation.as_ref().is_some_and(|readback| {
        readback.record().metadata().source_membership != member_receipt.member
    }) {
        return Err("NATIVE_PREPARATION_C02_MEMBER_CONFLICT");
    }

    let selected = retention::selected(input, journal, publisher, config)
        .map_err(|_| "NATIVE_PREPARATION_RETENTION_SELECTION_REFUSED")?;
    let claim = selected
        .claim
        .as_deref()
        .or(Some(original_profile.residency_closure.as_slice()));
    let domains = input
        .native
        .retention_domains(
            input.root,
            input.namespace,
            input.security,
            &selected.current,
            claim,
            input.deadline,
        )
        .map_err(|_| "NATIVE_PREPARATION_RESIDENCY_REFUSED")?;
    let residency_canonical =
        residency_canonical_bytes(&domains).map_err(|_| "NATIVE_PREPARATION_RESIDENCY_REFUSED")?;
    if residency_canonical != original_profile.residency_closure.as_slice() {
        return Err("NATIVE_PREPARATION_RESIDENCY_CONFLICT");
    }

    // Member setup may advance the journal generation. Re-read the real route
    // and the same original operation before selecting any projection IDs.
    let (current_preparation, current_snapshot) = read_c02_state(input, journal, publisher)?;
    match (&retained_preparation, &current_preparation) {
        (None, None) => {}
        (Some(first), Some(current)) if first.record() == current.record() => {}
        _ => return Err("NATIVE_PREPARATION_C02_OPERATION_RACED"),
    }
    if *current_snapshot.visibility() != initial_visibility {
        return Err("NATIVE_PREPARATION_ROUTE_CHANGED");
    }
    let visibility = *current_snapshot.visibility();
    let target_epoch = visibility
        .visible_epoch
        .checked_next()
        .map_err(|_| "NATIVE_PREPARATION_TARGET_EPOCH_REFUSED")?;
    let vector = NamedVectorRequirement::new(
        VectorName::new(RAW_CODE_VECTOR_NAME).map_err(|_| "NATIVE_PREPARATION_SCHEMA_INVALID")?,
        VectorRequirement::Sparse {
            index_ceiling: RAW_CODE_INDEX_CEILING,
            idf_enabled: true,
        },
    )
    .map_err(|_| "NATIVE_PREPARATION_SCHEMA_INVALID")?;
    let profile_set = raw_code_profile_set(&profiles, &vector)?;
    let schema = CollectionSchema::initial(
        journal.identity().installation_incarnation_id,
        visibility.collection_generation_id,
        profile_set.profile_set_id().clone(),
        std::slice::from_ref(&vector),
    )
    .map_err(|_| "NATIVE_PREPARATION_SCHEMA_INVALID")?;
    let schema_identity = schema
        .identity()
        .map_err(|_| "NATIVE_PREPARATION_SCHEMA_INVALID")?;
    if *schema_identity.as_digest() != visibility.schema_identity_digest {
        return Err("NATIVE_PREPARATION_SCHEMA_NOT_CURRENT");
    }
    let projection_schema_id = ProfileId::new(RAW_CODE_SCHEMA_PROFILE_ID)
        .map_err(|_| "NATIVE_PREPARATION_SCHEMA_INVALID")?;

    let mut port = original_port(journal, publisher, input.deadline, &work)
        .map_err(|_| "NATIVE_PREPARATION_C03_RESTORE_REFUSED")?;
    let registry = port
        .restore(input.native.context())
        .map_err(|_| "NATIVE_PREPARATION_C03_RESTORE_REFUSED")?
        .into_registry();
    let mut root_checkpoint = || {
        input
            .native
            .source_full_checkpoint()
            .map_err(|_| OwnerError::OwnerOperationConflict)
            .and_then(|()| {
                input
                    .security
                    .checkpoint()
                    .map_err(|_| OwnerError::OwnerOperationConflict)
            })
            .and_then(|()| {
                input.current_source.map_or(Ok(()), |check| {
                    check().map_err(|_| OwnerError::OwnerOperationConflict)
                })
            })
    };
    let root_scope = port
        .ensure_source_scope(
            &registry,
            input.root,
            input.native.context(),
            &mut root_checkpoint,
        )
        .map_err(|_| "NATIVE_PREPARATION_ROOT_SCOPE_REFUSED")?;
    let source_membership_id = member_receipt.member.source_membership_id;
    let (registered_member, registered_source) = registry
        .active_source_membership_v2(source_membership_id)
        .map_err(|_| "NATIVE_PREPARATION_MEMBER_NOT_ACTIVE")?;
    if registered_member != &member_receipt.member
        || registered_source != input.source.identity()
        || root_scope.workspace_id != registered_member.workspace_id
        || root_scope.corpus_id != registered_member.corpus_id
    {
        return Err("NATIVE_PREPARATION_MEMBER_SCOPE_MISMATCH");
    }
    let permanent = registry
        .permanent_membership(source_membership_id)
        .map_err(|_| "NATIVE_PREPARATION_MEMBER_PROVENANCE_MISSING")?;
    let provenance = permanent.provenance().clone();
    let semantics = partition_semantics(
        root_scope.workspace_id.as_bytes(),
        root_scope.corpus_id.as_bytes(),
        input,
        &domains,
        &profiles,
        &profile_set,
        &vector,
    )?;
    let (projection_operation_id, projection_receipt) =
        projection_operation(&input.command.operation, source_membership_id)?;
    let operation_lookup = port
        .load_batch(&projection_operation_id, input.native.context())
        .map_err(|_| "NATIVE_PREPARATION_PROJECTION_OPERATION_READ_REFUSED")?;
    let prior_projection = match registry.projection_membership_record(source_membership_id) {
        Ok(record) => Some(record.clone()),
        Err(RegistryError::ProjectionMembershipNotFound) => None,
        Err(_) => return Err("NATIVE_PREPARATION_PROJECTION_READ_REFUSED"),
    };
    match (
        retained_preparation.as_ref(),
        prior_projection.as_ref(),
        &operation_lookup,
    ) {
        (Some(_), Some(_), RegistryBatchLookup::Committed(_))
        | (Some(_), None, RegistryBatchLookup::Absent { .. })
        | (None, None, RegistryBatchLookup::Absent { .. }) => {}
        _ => return Err("NATIVE_PREPARATION_PROJECTION_ORIGINAL_CONFLICT"),
    }
    if let Some(record) = &prior_projection {
        if retained_preparation.is_none()
            || record.binding()
                != &retained_preparation
                    .as_ref()
                    .expect("checked above")
                    .record()
                    .metadata()
                    .projection_membership
            || record.request().partition_semantics.as_ref() != Some(&semantics)
        {
            return Err("NATIVE_PREPARATION_PROJECTION_BINDING_CONFLICT");
        }
        read_projection_membership(&registry, source_membership_id, registry.revision())
            .map_err(|_| "NATIVE_PREPARATION_PROJECTION_OWNER_FENCE_REFUSED")?;
    }
    drop(port);

    let (materialization_id, representation_id, projection_membership) =
        if let Some(readback) = &retained_preparation {
            let metadata = readback.record().metadata();
            (
                metadata.materialization.materialization_id,
                metadata.representation.representation_id,
                metadata.projection_membership.clone(),
            )
        } else {
            let scan = semantics
                .access()
                .len()
                .checked_add(semantics.scoring().len())
                .and_then(|bytes| {
                    search_source_registry::api::DEFAULT_REGISTRY_LIMITS
                        .max_memberships
                        .checked_mul(bytes)
                })
                .ok_or("NATIVE_PREPARATION_PARTITION_SCAN_BOUNDS")?;
            work(Work::Process(scan)).map_err(|_| "NATIVE_PREPARATION_ORIGINAL_CONTEXT_REFUSED")?;
            let reusable =
                search_source_registry::reusable_projection_partitions(&registry, &semantics)
                    .map_err(|_| "NATIVE_PREPARATION_PARTITION_REUSE_REFUSED")?;
            let ids = SourceIdentityIdsV2::<
                super::super::read_child::fresh_root::FreshRootCancellation,
            >::bind(input.deadline, &work);
            let materialization_id = ids
                .fresh_materialization_id(input.native.context())
                .map_err(|_| "NATIVE_PREPARATION_MATERIALIZATION_ID_REFUSED")?;
            let representation_id = ids
                .fresh_representation_id(input.native.context())
                .map_err(|_| "NATIVE_PREPARATION_REPRESENTATION_ID_REFUSED")?;
            let projection_membership_id = ids
                .fresh_projection_membership_id(input.native.context())
                .map_err(|_| "NATIVE_PREPARATION_PROJECTION_ID_REFUSED")?;
            let access_partition_id = match reusable.access() {
                Some(id) => id,
                None => ids
                    .fresh_access_partition_id(input.native.context())
                    .map_err(|_| "NATIVE_PREPARATION_ACCESS_PARTITION_ID_REFUSED")?,
            };
            let scoring_partition_id = match reusable.scoring() {
                Some(id) => id,
                None => ids
                    .fresh_scoring_partition_id(input.native.context())
                    .map_err(|_| "NATIVE_PREPARATION_SCORING_PARTITION_ID_REFUSED")?,
            };
            (
                materialization_id,
                representation_id,
                ProjectionMembership {
                    projection_membership_id,
                    source_membership_id,
                    representation_id,
                    access_partition_id,
                    scoring_partition_id,
                    projection_schema_id: projection_schema_id.clone(),
                },
            )
        };
    if projection_membership.source_membership_id != source_membership_id
        || projection_membership.representation_id != representation_id
        || projection_membership.projection_schema_id != projection_schema_id
    {
        return Err("NATIVE_PREPARATION_PROJECTION_BINDING_INVALID");
    }
    let c03_request = ProjectionMembershipBindRequest {
        binding: projection_membership.clone(),
        expected_membership_revision: registered_member.membership_revision,
        expected_provenance: provenance,
        partition_semantics: Some(semantics),
    };
    let owners = prepare_retained_source_owners(
        input,
        journal,
        publisher,
        config,
        &profiles,
        &source_revision,
        materialization_id,
        representation_id,
    )?;

    let mut c02_metadata = PreparationInputMetadataV1 {
        original_operation_id: input.command.operation.clone(),
        c02_mutation_id: MutationId([0; 32]),
        expected_generation: current_snapshot.generation(),
        source_revision,
        source_membership: member_receipt.member,
        materialization: owners.materialization.clone(),
        representation: owners.representation.clone(),
        projection_membership: projection_membership.clone(),
        collection_schema: schema.clone(),
        residency_canonical,
        policy_canonical: input.security.policy_body().to_vec(),
    };
    let expected_preparation = if let Some(readback) = &retained_preparation {
        let prior = readback.record().metadata();
        c02_metadata.c02_mutation_id = readback.record().c02_mutation_id();
        c02_metadata.expected_generation = readback.record().expected_generation();
        if &c02_metadata != prior {
            return Err("NATIVE_PREPARATION_C02_INPUT_CONFLICT");
        }
        readback.record().clone()
    } else {
        let ids = SourceIdentityIdsV2::<
            super::super::read_child::fresh_root::FreshRootCancellation,
        >::bind(input.deadline, &work);
        c02_metadata.c02_mutation_id = ids
            .fresh_preparation_mutation_id(input.native.context())
            .map_err(|_| "NATIVE_PREPARATION_C02_ID_REFUSED")?;
        let record = PreparationInputRecordV1::prepare_with_original_context(
            c02_metadata,
            journal.limits(),
            input.deadline,
            &work,
        )
        .map_err(|_| "NATIVE_PREPARATION_C02_INPUT_REFUSED")?;
        commit_c02_input(input, journal, publisher, &record)?;
        record
    };

    let bound = bind_projection_exact(
        input,
        journal,
        publisher,
        c03_request,
        projection_operation_id,
        projection_receipt,
    )?;
    if bound.binding != expected_preparation.metadata().projection_membership {
        return Err("NATIVE_PREPARATION_PROJECTION_READBACK_MISMATCH");
    }
    let (post_preparation, post_snapshot) = read_c02_state(input, journal, publisher)?;
    if post_preparation
        .as_ref()
        .map(PreparationInputReadbackV1::record)
        != Some(&expected_preparation)
        || *post_snapshot.visibility() != initial_visibility
    {
        return Err("NATIVE_PREPARATION_C02_POST_BIND_READBACK_MISMATCH");
    }
    if post_snapshot
        .shadows()
        .contains_key(&projection_membership.projection_membership_id)
    {
        return Err("NATIVE_PREPARATION_PROJECTION_SHADOWED");
    }
    if post_snapshot
        .manifests()
        .contains_key(&projection_membership.projection_membership_id)
        || post_snapshot
            .prepared_artifact_sets()
            .contains_key(&projection_membership.projection_membership_id)
    {
        return Err("NATIVE_PREPARATION_PROJECTION_ALREADY_REFERENCED");
    }

    let scope = PlanningScope {
        membership: ProjectionMembershipBinding {
            projection_membership_id: projection_membership.projection_membership_id,
            source_membership_id,
            source_id: source_revision.source_id,
            source_revision_id: source_revision.revision_id,
            representation_id,
            projection_schema_id,
            collection_generation_id: visibility.collection_generation_id,
            installation_incarnation_id: journal.identity().installation_incarnation_id,
            projection_profile_set_id: profile_set.profile_set_id().clone(),
            access_partition_id: projection_membership.access_partition_id,
            scoring_partition_id: projection_membership.scoring_partition_id,
        },
        valid_from_epoch: target_epoch,
        valid_until_epoch_exclusive: None,
        lineage: LineageObservation {
            repository_lineage_id: None,
        },
        identity_limits: PointIdentityLimits::default(),
    };
    let cancelled = input.native.original_cancellation_flag();
    let unitization_budget = UnitizationBudget {
        max_steps: profiles.unitizer.descriptor().max_steps,
        max_encoded_bytes: profiles.unitizer.descriptor().max_manifest_bytes,
        deadline: input.deadline,
        cancelled,
    };
    let materialization_budget = DEFAULT_MATERIALIZATION_BUDGET;
    let max_input_bytes = usize::try_from(materialization_budget.max_input_bytes)
        .map_err(|_| "NATIVE_PREPARATION_RESOURCE_BOUNDS")?
        .min(profiles.unitizer.descriptor().limits.max_input_bytes);
    let max_artifact_bytes = usize::try_from(materialization_budget.max_output_bytes)
        .map_err(|_| "NATIVE_PREPARATION_RESOURCE_BOUNDS")?;
    let language =
        c::BoundedName::new("code").map_err(|_| "NATIVE_PREPARATION_LANGUAGE_INVALID")?;
    let binding = V3SourceBinding {
        source_namespace_id: source_revision.source_namespace_id,
        source_id: source_revision.source_id,
        source_revision_id: source_revision.revision_id,
        representation_id,
        materialization_id,
    };
    let producer = RawCodeArtifactProducerInput {
        binding: &binding,
        request: &owners.request,
        materializer: &profiles.materializer,
        unitizer: &profiles.unitizer,
        materialization_budget,
        unitization_budget: &unitization_budget,
        scope: &scope,
        schema: &schema,
        lexical_profile: &profiles.lexical,
        language: &language,
        lexical_limits: CodeIdentifierLimits::HARD,
        sparse_limits: SparseLimits::BASELINE,
        projection_budget: ProjectionBudget::BASELINE,
        max_input_bytes,
        max_total_vector_values: usize::try_from(MAX_STORED_VECTOR_VALUES)
            .map_err(|_| "NATIVE_PREPARATION_RESOURCE_BOUNDS")?,
        max_artifact_bytes,
    };
    let expected_metadata = expected_preparation.metadata().clone();
    let result = super::artifact_store::store_native_raw_code_artifacts(
        input,
        journal,
        publisher,
        config,
        &producer,
        |prepared| {
            let materialization = materialization_from_product(
                prepared.product(),
                &source_revision,
                materialization_id,
                original_result.revision.object_residency_key_digest,
                &profiles.materializer,
            )
            .map_err(|_| {
                crate::preparation_composition::native_artifact_store::NativeArtifactCompositionError::Native(
                    crate::owner_composition::NativeArtifactStoreError::OwnerRefused,
                )
            })?;
            let representation = representation_from_units(
                prepared.units(),
                materialization_id,
                representation_id,
                &profiles.unitizer,
            )
            .map_err(|_| {
                crate::preparation_composition::native_artifact_store::NativeArtifactCompositionError::Native(
                    crate::owner_composition::NativeArtifactStoreError::OwnerRefused,
                )
            })?;
            if materialization != expected_metadata.materialization
                || representation != expected_metadata.representation
                || prepared.scope() != &scope
                || prepared.schema() != &schema
            {
                return Err(
                    crate::preparation_composition::native_artifact_store::NativeArtifactCompositionError::Native(
                        crate::owner_composition::NativeArtifactStoreError::OwnerRefused,
                    ),
                );
            }
            Ok(())
        },
    )
    .map_err(|error| {
        if error.outcome_unknown() {
            "NATIVE_PREPARATION_C07_OUTCOME_UNKNOWN"
        } else {
            "NATIVE_PREPARATION_C07_STORE_REFUSED"
        }
    })?;
    Ok(result)
}

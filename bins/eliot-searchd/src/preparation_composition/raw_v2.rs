use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, SourceRevisionRef};
use search_materializer::api::{
    CancellationToken, MaterializationBudget, MaterializationContext, MaterializationError,
    MaterializationProduct, RevisionReadPort, StoredRevisionBytes, ValidatedMaterializationRequest,
    ValidatedMaterializerProfile, materialize_text_or_code,
};
use search_unitizer::{
    UnitizationBudget, UnitizationError, V3SourceBinding, ValidatedV3UnitizerProfile,
    VerifiedUnitSet, build_unit_manifest, prepare_unit_set_input,
};
use std::{
    cell::Cell,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RawPreparationError {
    Materialization(MaterializationError),
    Unitization(UnitizationError),
}

impl From<MaterializationError> for RawPreparationError {
    fn from(error: MaterializationError) -> Self {
        Self::Materialization(error)
    }
}

impl From<UnitizationError> for RawPreparationError {
    fn from(error: UnitizationError) -> Self {
        Self::Unitization(error)
    }
}

struct BoundBytes<'a> {
    body: &'a [u8],
    source_id: &'a OpaqueId,
    revision: NonZeroRevision,
    byte_count: u64,
    digest: Blake3Digest32,
    residency: &'a OpaqueId,
    cancelled: &'a AtomicBool,
    deadline: Instant,
    read: Cell<bool>,
}

impl RevisionReadPort for BoundBytes<'_> {
    fn read_exact(
        &self,
        source_id: &OpaqueId,
        revision: NonZeroRevision,
        byte_count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError> {
        if source_id != self.source_id
            || revision != self.revision
            || byte_count != self.byte_count
            || self.read.replace(true)
        {
            return Err(MaterializationError::RevisionUnavailable);
        }
        if self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            return Err(MaterializationError::Cancelled);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.body.len())
            .map_err(|_| MaterializationError::BudgetExhausted)?;
        bytes.extend_from_slice(self.body);
        Ok(StoredRevisionBytes::new(
            bytes,
            self.digest,
            self.residency.clone(),
        ))
    }
}

/// Materialize exact retained UTF-8 bytes and build the owners' opaque v3 `UnitSet`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_raw_v2(
    source: &SourceRevisionRef,
    occurrence_sequence: u64,
    retained_body: &[u8],
    binding: &V3SourceBinding,
    request: &ValidatedMaterializationRequest,
    materializer_profile: &ValidatedMaterializerProfile,
    unitizer_profile: &ValidatedV3UnitizerProfile,
    materialization_budget: MaterializationBudget,
    unitization_budget: &UnitizationBudget<'_>,
) -> Result<(MaterializationProduct, VerifiedUnitSet), RawPreparationError> {
    let source_revision_id = source.revision_id;
    if binding.source_namespace_id != source.source_namespace_id
        || binding.source_id != source.source_id
        || binding.source_revision_id != source_revision_id
    {
        return Err(UnitizationError::UnitManifestIncomplete.into());
    }
    let revision = NonZeroRevision::new(occurrence_sequence)
        .map_err(|_| MaterializationError::RequestInvalid)?;
    let body_bytes =
        u64::try_from(retained_body.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    if request.source_id().as_str() != source.source_id.to_string()
        || request.revision() != revision
        || request.admitted_unsaved()
    {
        return Err(MaterializationError::RevisionUnavailable.into());
    }
    if request.content_digest() != source.content_digest {
        return Err(MaterializationError::RevisionDigestMismatch.into());
    }
    if source.byte_length != body_bytes || request.byte_count() != body_bytes {
        return Err(MaterializationError::InputLengthMismatch.into());
    }
    let materialization_budget = materialization_budget.validate()?;
    if body_bytes > materialization_budget.max_input_bytes
        || body_bytes > materialization_budget.max_output_bytes
        || retained_body.len() > unitizer_profile.limits().max_input_bytes
    {
        return Err(MaterializationError::BudgetExhausted.into());
    }
    if unitization_budget.max_steps == 0
        || unitization_budget.max_steps > unitizer_profile.descriptor().max_steps
        || unitization_budget.max_encoded_bytes == 0
        || unitization_budget.max_encoded_bytes > unitizer_profile.descriptor().max_manifest_bytes
    {
        return Err(UnitizationError::InvalidLimits.into());
    }
    let remaining_steps = materialization_budget
        .max_steps
        .checked_sub(body_bytes)
        .filter(|steps| *steps > 0)
        .ok_or(MaterializationError::BudgetExhausted)?;
    let port = BoundBytes {
        body: retained_body,
        source_id: request.source_id(),
        revision,
        byte_count: body_bytes,
        digest: source.content_digest,
        residency: request.residency(),
        cancelled: unitization_budget.cancelled,
        deadline: unitization_budget.deadline,
        read: Cell::new(false),
    };
    let context = MaterializationContext {
        profile: materializer_profile.clone(),
        budget: MaterializationBudget {
            max_steps: remaining_steps,
            ..materialization_budget
        },
        cancel: CancellationToken::new(unitization_budget.cancelled),
    };
    let product = match materialize_text_or_code(request, &port, &context) {
        Ok(product) => product,
        Err(MaterializationError::Cancelled) if Instant::now() >= unitization_budget.deadline => {
            return Err(UnitizationError::DeadlineExceeded.into());
        }
        Err(error) => return Err(error.into()),
    };
    let input = prepare_unit_set_input(
        (*binding).clone(),
        &product,
        request,
        materializer_profile,
        unitizer_profile,
        unitization_budget,
    )?;
    let set = build_unit_manifest(&input, unitizer_profile, unitization_budget)?;
    let provenance = set.manifest().provenance();
    if provenance.binding() != binding
        || provenance.input_digest() != source.content_digest
        || provenance.native_bytes() != source.byte_length
        || provenance.legacy_revision_sequence() != occurrence_sequence
    {
        return Err(UnitizationError::UnitManifestIncomplete.into());
    }
    Ok((product, set))
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{
        MaterializationId, ProfileId, RepresentationId, SourceId, SourceNamespaceId,
        SourceRevisionId, UnitKind,
    };
    use search_materializer::api::{
        AcceptedProfiles, BomPolicy, LossBehavior, MaterializationRequest,
        MaterializerProfileDescriptor, NewlinePolicy, SourceEncoding, SourceKind,
        baseline_profile_descriptor, validate_materialization_request,
        validate_materializer_profile,
    };
    use search_unitizer::{
        UnitizationLimits, V3AnchorPolicy, V3AttachmentPolicy, V3EmptyPolicy, V3OmissionPolicy,
        V3OverlapPolicy, V3RepresentationKind, V3UnitizerProfileDescriptor,
        validate_v3_unitizer_profile,
    };
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    const MATERIALIZATION_BUDGET: MaterializationBudget = MaterializationBudget {
        max_input_bytes: 4096,
        max_output_bytes: 4096,
        max_lines: 512,
        max_map_segments: 512,
        max_loss_records: 512,
        max_steps: 65_536,
    };

    struct Fixture {
        source: SourceRevisionRef,
        sequence: u64,
        binding: V3SourceBinding,
        request: ValidatedMaterializationRequest,
        materializer: ValidatedMaterializerProfile,
        unitizer: ValidatedV3UnitizerProfile,
    }

    fn digest(bytes: &[u8]) -> Blake3Digest32 {
        Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes())
    }

    fn fixture(body: &[u8], newline: NewlinePolicy, bom: BomPolicy) -> Fixture {
        let source_namespace_id = SourceNamespaceId::from_bytes([1; 16]);
        let source_id = SourceId::from_bytes([2; 16]);
        let source_revision_id = SourceRevisionId::from_bytes([3; 16]);
        let source = SourceRevisionRef {
            source_namespace_id,
            source_id,
            revision_id: source_revision_id,
            content_digest: digest(body),
            byte_length: body.len() as u64,
        };
        let binding = V3SourceBinding {
            source_namespace_id,
            source_id,
            source_revision_id,
            materialization_id: MaterializationId::from_bytes([4; 16]),
            representation_id: RepresentationId::from_bytes([5; 16]),
        };
        let mut descriptor: MaterializerProfileDescriptor =
            baseline_profile_descriptor("raw-v2-test", 1);
        descriptor.source_kinds = vec![SourceKind::Text];
        descriptor.encodings = vec![SourceEncoding::Utf8];
        descriptor.bom_policy = bom;
        descriptor.newline_policy = newline;
        descriptor.loss_behavior = LossBehavior::RecordAndLowerAssurance;
        let materializer = validate_materializer_profile(&descriptor).unwrap();
        let request = MaterializationRequest {
            source_id: OpaqueId::new(source_id.to_string()).unwrap(),
            revision: NonZeroRevision::new(7).unwrap(),
            residency: OpaqueId::new("raw-v2-test-retained-object").unwrap(),
            content_digest: source.content_digest,
            byte_count: source.byte_length,
            declared_kind: SourceKind::Text,
            declared_encoding: SourceEncoding::Utf8,
            profile_id: materializer.id(),
            operation_id: OpaqueId::new("raw-v2-test-operation").unwrap(),
            from_unsaved_bytes: false,
            unsaved_snapshot_receipt: None,
        };
        let request = validate_materialization_request(
            &request,
            &AcceptedProfiles::new(vec![materializer.clone()]),
            &MATERIALIZATION_BUDGET,
        )
        .unwrap();
        let unitizer = validate_v3_unitizer_profile(&V3UnitizerProfileDescriptor {
            profile_name: ProfileId::new("raw-v2-test-unitizer").unwrap(),
            profile_revision: 1,
            representation_kind: V3RepresentationKind::Text,
            unit_kind: UnitKind::File,
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
            max_manifest_bytes: 65_536,
            max_steps: 65_536,
            fixture_digest: Blake3Digest32::from_bytes([7; 32]),
        })
        .unwrap();
        Fixture {
            source,
            sequence: 7,
            binding,
            request,
            materializer,
            unitizer,
        }
    }

    fn unit_budget(cancelled: &AtomicBool) -> UnitizationBudget<'_> {
        UnitizationBudget {
            max_steps: 65_536,
            max_encoded_bytes: 65_536,
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled,
        }
    }

    #[test]
    fn exact_retained_crlf_bytes_produce_owner_verified_unit_set() {
        let body = b"first\r\nsecond\n";
        let f = fixture(
            body,
            NewlinePolicy::PreserveExact,
            BomPolicy::RejectWhenPresent,
        );
        let cancelled = AtomicBool::new(false);
        let (product, set) = prepare_raw_v2(
            &f.source,
            f.sequence,
            body,
            &f.binding,
            &f.request,
            &f.materializer,
            &f.unitizer,
            MATERIALIZATION_BUDGET,
            &unit_budget(&cancelled),
        )
        .unwrap();
        assert_eq!(product.canonical_text().as_bytes(), body);
        assert_eq!(set.manifest().provenance().binding(), &f.binding);
        assert_eq!(
            set.manifest().provenance().input_digest(),
            f.source.content_digest
        );
        assert_eq!(
            set.manifest().provenance().native_bytes(),
            f.source.byte_length
        );
        assert_eq!(
            set.manifest().provenance().legacy_revision_sequence(),
            f.sequence
        );
        assert!(!set.units().is_empty());
    }

    #[test]
    fn wrong_source_digest_is_refused_before_materialization() {
        let body = b"retained text\n";
        let f = fixture(
            body,
            NewlinePolicy::PreserveExact,
            BomPolicy::RejectWhenPresent,
        );
        let mut wrong = f.source;
        wrong.content_digest = digest(b"different body\n");
        let cancelled = AtomicBool::new(false);
        assert_eq!(
            prepare_raw_v2(
                &wrong,
                f.sequence,
                body,
                &f.binding,
                &f.request,
                &f.materializer,
                &f.unitizer,
                MATERIALIZATION_BUDGET,
                &unit_budget(&cancelled),
            ),
            Err(RawPreparationError::Materialization(
                MaterializationError::RevisionDigestMismatch
            ))
        );
    }

    #[test]
    fn typed_binding_must_match_the_source_revision_reference() {
        let body = b"retained text\n";
        let f = fixture(
            body,
            NewlinePolicy::PreserveExact,
            BomPolicy::RejectWhenPresent,
        );
        let mut binding = f.binding;
        binding.source_revision_id = SourceRevisionId::from_bytes([9; 16]);
        let cancelled = AtomicBool::new(false);
        assert_eq!(
            prepare_raw_v2(
                &f.source,
                f.sequence,
                body,
                &binding,
                &f.request,
                &f.materializer,
                &f.unitizer,
                MATERIALIZATION_BUDGET,
                &unit_budget(&cancelled),
            ),
            Err(RawPreparationError::Unitization(
                UnitizationError::UnitManifestIncomplete
            ))
        );
    }

    #[test]
    fn retained_body_range_must_match_reference_and_request() {
        let body = b"retained text\n";
        let f = fixture(
            body,
            NewlinePolicy::PreserveExact,
            BomPolicy::RejectWhenPresent,
        );
        let cancelled = AtomicBool::new(false);
        assert_eq!(
            prepare_raw_v2(
                &f.source,
                f.sequence,
                &body[..body.len() - 1],
                &f.binding,
                &f.request,
                &f.materializer,
                &f.unitizer,
                MATERIALIZATION_BUDGET,
                &unit_budget(&cancelled),
            ),
            Err(RawPreparationError::Materialization(
                MaterializationError::InputLengthMismatch
            ))
        );
    }

    #[test]
    fn bom_or_newline_transform_cannot_be_sealed_as_exact_raw_units() {
        let body = b"\xef\xbb\xbffirst\r\nsecond\n";
        let f = fixture(
            body,
            NewlinePolicy::NormalizeToLf,
            BomPolicy::StripAndRecord,
        );
        let cancelled = AtomicBool::new(false);
        assert_eq!(
            prepare_raw_v2(
                &f.source,
                f.sequence,
                body,
                &f.binding,
                &f.request,
                &f.materializer,
                &f.unitizer,
                MATERIALIZATION_BUDGET,
                &unit_budget(&cancelled),
            ),
            Err(RawPreparationError::Unitization(
                UnitizationError::UnitManifestIncomplete
            ))
        );
    }
}

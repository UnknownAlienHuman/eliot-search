//! Staged pre-seal occurrence reservation for source revisions.
//!
//! This module provides the durable, typed pre-seal profile that persists the
//! complete occurrence metadata BEFORE the AES sealing step. The final physical
//! object-residency digest does not exist until AES completes; it is bound
//! afterwards by a pure construction helper, and this module never fills the
//! old SourceRevision with a dummy digest.
//!
//! Two properties are load-bearing:
//!
//! * The complete original pre-seal descriptor is retained immutably through
//!   SourceRevisionPreparePort and is NOT removed by the later commit. The
//!   one-time object commit remains the control adapter compare-and-set, never
//!   a side effect of a pure function here, and this module has no destructive
//!   clear path.
//! * A committed operation is reported as an exact committed replay only when
//!   the retained original descriptor and the final operation readback agree on
//!   every original field. The legacy final readback alone does not prove the
//!   residency closure, so a missing or non-matching retained original fails
//!   closed instead of being relabelled from the older record shape.
//!
//! The new profile is unshipped, so it uses exactly one encoding owner: the
//! shared canonical CBOR encoder and parser in search-contracts. The frozen
//! revision.rs record and request codecs are untouched.

use search_contracts::{
    AcquisitionKind, Blake3Digest32, BoundedBytes, BoundedMap, CanonicalKey, CanonicalText,
    CanonicalValue, ClosedCanonicalObject, ContractError, MAX_MAP_ENTRIES,
    ObjectResidencyKeyDigest, OpaqueCanonicalBytes, OpaqueId, ReceiptRef, SourceId,
    SourceNamespaceId, SourceRevision, SourceRevisionId, UtcTimestamp, parse_canonical_cbor,
    to_canonical_cbor,
};
use search_ports::{CancellationProbe, OperationContext, Port};

use crate::error::{RegistryError, cancelled_before_commit};
use crate::revision::{
    SourceRevisionControlPort, SourceRevisionIdPort, SourceRevisionMutationReadback,
    SourceRevisionSourceState, is_fresh_uuid_v4, load_head, load_operation, load_revision,
    validate_expected_head_parts,
};

/// Frozen header of the pre-seal profile encoding.
const PRESEAL_PROFILE_HEADER: &str = "eliot-search/source-revision-preseal/v1";
/// Schema revision of the pre-seal profile encoding.
const PRESEAL_PROFILE_SCHEMA: u64 = 1;

/// Exact byte ceiling for one complete encoded pre-seal profile.
///
/// The bound covers the complete profile including the residency closure and
/// matches the finite control/AAD budget the sealed operation must fit into.
/// It is enforced BEFORE any occurrence UUID is drawn, so oversized input
/// cannot consume an identity or a nonce reservation.
pub const MAX_SOURCE_REVISION_PRESEAL_PROFILE_BYTES: usize = 8_192;

/// Bounded port for exact pre-seal profile retention and resolution.
///
/// The control adapter owns persistence. It must durably retain the complete
/// original descriptor for one operation identity and must keep that descriptor
/// readable after the final object commit, because the committed replay path
/// proves the residency closure from it. Retaining a different descriptor under
/// the same operation identity must be refused without overwriting.
///
/// This trait is the only durable authority for pre-seal state; a metadata
/// struct or a decoder alone is not durable authority.
pub trait SourceRevisionPreparePort: Port {
    /// Durably retains one complete original pre-seal profile.
    ///
    /// Returns Ok only when the exact profile is durably retained, either by
    /// this call or by an earlier identical retention. A different profile for
    /// the same operation identity returns an error and overwrites nothing.
    fn retain_source_revision_preseal_profile(
        &mut self,
        profile: &SourceRevisionPresealProfile,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<(), Self::Error>;

    /// Loads the exact retained pre-seal profile for one operation, if any.
    ///
    /// The retained descriptor stays readable after the final commit; this call
    /// never removes it.
    fn load_source_revision_preseal_profile(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevisionPresealProfile>, Self::Error>;
}

/// Additive pre-seal request that excludes the final physical object digest.
///
/// This is the input to prepare_source_revision. It carries the complete
/// canonical residency closure as canonical CBOR bytes, never the final object
/// digest, which does not exist before AES. The older
/// SourceRevisionRegistrationRequest is unchanged and remains the post-seal
/// registration input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionPresealRequest {
    /// Stable idempotency identity.
    pub operation_id: OpaqueId,
    /// Namespace that durably admitted the source.
    pub source_namespace_id: SourceNamespaceId,
    /// Accepted source identity.
    pub source_id: SourceId,
    /// Exact expected current revision, or None for the first occurrence.
    pub expected_head: Option<SourceRevisionId>,
    /// Digest of the stable read or immutable object accepted by the caller.
    pub content_digest: Blake3Digest32,
    /// Exact byte length bound to content_digest.
    pub byte_length: u64,
    /// Trusted observation time supplied by the acquisition owner.
    pub observed_at: UtcTimestamp,
    /// Closed acquisition source class.
    pub acquisition_kind: AcquisitionKind,
    /// Content-free proof reference for the accepted stable read.
    pub stability_receipt_ref: ReceiptRef,
    /// Complete canonical CBOR residency closure bytes.
    ///
    /// These are metadata bodies and digests, not the final physical
    /// object-residency digest, which is unknown before AES. Non-canonical or
    /// oversized bytes are refused.
    pub residency_closure: Vec<u8>,
}

impl SourceRevisionPresealRequest {
    /// Creates one pre-seal request.
    #[must_use]
    pub fn new(
        operation_id: OpaqueId,
        source_namespace_id: SourceNamespaceId,
        source_id: SourceId,
        expected_head: Option<SourceRevisionId>,
        content_digest: Blake3Digest32,
        byte_length: u64,
        observed_at: UtcTimestamp,
        acquisition_kind: AcquisitionKind,
        stability_receipt_ref: ReceiptRef,
        residency_closure: Vec<u8>,
    ) -> Self {
        Self {
            operation_id,
            source_namespace_id,
            source_id,
            expected_head,
            content_digest,
            byte_length,
            observed_at,
            acquisition_kind,
            stability_receipt_ref,
            residency_closure,
        }
    }
}

/// Complete typed pre-seal occurrence profile retained before AES sealing.
///
/// It carries the reserved occurrence UUID and sequence, the exact expected
/// head, the accepted content digest and length, the observation time, the
/// stability receipt and the canonical residency closure. The final physical
/// object-residency digest is deliberately absent: it is produced after AES by
/// late_bind_object_residency and committed by the control adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionPresealProfile {
    /// Stable idempotency identity.
    pub operation_id: OpaqueId,
    /// Namespace that durably admitted the source.
    pub source_namespace_id: SourceNamespaceId,
    /// Accepted source identity.
    pub source_id: SourceId,
    /// Exact expected current revision, or None for the first occurrence.
    pub expected_head: Option<SourceRevisionId>,
    /// Occurrence UUID reserved exactly once for this operation.
    pub revision_id: SourceRevisionId,
    /// Occurrence sequence derived from the exact durable head.
    pub occurrence_sequence: u64,
    /// Digest of the stable read or immutable object accepted by the caller.
    pub content_digest: Blake3Digest32,
    /// Exact byte length bound to content_digest.
    pub byte_length: u64,
    /// Trusted observation time supplied by the acquisition owner.
    pub observed_at: UtcTimestamp,
    /// Closed acquisition source class.
    pub acquisition_kind: AcquisitionKind,
    /// Content-free proof reference for the accepted stable read.
    pub stability_receipt_ref: ReceiptRef,
    /// Complete canonical CBOR residency closure bytes.
    pub residency_closure: OpaqueCanonicalBytes,
}

/// Outcome of resolving one operation through the pre-seal path.
///
/// The two variants are distinct on purpose: a staged occurrence that has not
/// been committed must not be reported as an exact committed replay, and a
/// committed occurrence is only reported when the retained original descriptor
/// proves the full original intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceRevisionPreparation {
    /// The operation already has a committed final occurrence.
    Committed(SourceRevisionCommittedPreparation),
    /// The operation has a staged pre-seal occurrence with no final commit.
    Pending(SourceRevisionPendingPreparation),
}

/// Exact committed replay resolved from the retained original and readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionCommittedPreparation {
    /// Complete original retained pre-seal descriptor.
    pub profile: SourceRevisionPresealProfile,
    /// Final committed operation readback, including the object residency
    /// digest produced after AES.
    pub readback: SourceRevisionMutationReadback,
}

/// Staged pre-seal occurrence resolved without a final commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionPendingPreparation {
    /// Exact retained pre-seal descriptor.
    pub profile: SourceRevisionPresealProfile,
    /// Whether an existing retained descriptor was returned unchanged.
    pub replayed: bool,
}

/// Resolves or stages one pre-seal occurrence for one operation.
///
/// The order is fixed and load-bearing:
///
/// 1. The canonical residency closure and the complete profile bound are
///    checked before any durable read, observation time or UUID draw.
/// 2. An already committed operation is resolved from the retained original
///    descriptor AND the final readback; a missing original fails closed.
/// 3. An existing retained descriptor replays unchanged, without another UUID.
/// 4. Only then are the durable head, the occurrence UUID, the sequence and the
///    retained descriptor produced.
///
/// # Errors
/// Returns a closed RegistryError: record-invalid for non-canonical or
/// oversized input, operation-conflict for a different request under a used
/// operation identity, outcome-unknown for a committed operation whose retained
/// original is missing or disagrees with the final readback, and the existing
/// head/source errors from the shared evaluator.
pub fn prepare_source_revision<C, I, P>(
    request: &SourceRevisionPresealRequest,
    id_port: &mut I,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceRevisionPreparation, RegistryError>
where
    C: CancellationProbe,
    I: SourceRevisionIdPort<Cancellation = C>,
    P: SourceRevisionControlPort<Cancellation = C> + SourceRevisionPreparePort<Cancellation = C>,
{
    cancelled_before_commit(context)?;

    let closure = canonical_residency_closure(&request.residency_closure)?;
    check_preseal_profile_bound(request, &closure)?;

    if let Some(readback) = load_operation(control_port, &request.operation_id, context)? {
        let original = load_retained_profile(control_port, &request.operation_id, context)?
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
        verify_committed_preparation(&original, &readback, request, &closure)?;
        return Ok(SourceRevisionPreparation::Committed(
            SourceRevisionCommittedPreparation {
                profile: original,
                readback,
            },
        ));
    }

    if let Some(existing) = load_retained_profile(control_port, &request.operation_id, context)? {
        if staged_profile_matches_request(&existing, request, &closure)? {
            return Ok(SourceRevisionPreparation::Pending(
                SourceRevisionPendingPreparation {
                    profile: existing,
                    replayed: true,
                },
            ));
        }
        return Err(RegistryError::OperationConflict);
    }

    let head = load_head(
        control_port,
        &request.source_namespace_id,
        &request.source_id,
        context,
    )?
    .ok_or(RegistryError::SourceNotFound)?;
    let previous_revision = validate_expected_head_parts(
        request.source_namespace_id,
        request.source_id,
        request.expected_head,
        &head,
    )?;
    if head.state != SourceRevisionSourceState::Active {
        return Err(RegistryError::SourceRetired);
    }

    cancelled_before_commit(context)?;
    let revision_id = id_port
        .fresh_source_revision_id(context)
        .map_err(|_| RegistryError::RevisionIdUnavailable)?;
    if !is_fresh_uuid_v4(&revision_id) {
        return Err(RegistryError::RevisionIdUnavailable);
    }
    if load_revision(
        control_port,
        &request.source_namespace_id,
        &request.source_id,
        &revision_id,
        context,
    )?
    .is_some()
    {
        return Err(RegistryError::SourceRevisionIdCollision);
    }

    let occurrence_sequence = previous_revision.map_or(Ok(1), |revision| {
        revision
            .occurrence_sequence
            .checked_add(1)
            .ok_or(RegistryError::ContractExhausted)
    })?;

    let profile = SourceRevisionPresealProfile {
        operation_id: request.operation_id.clone(),
        source_namespace_id: request.source_namespace_id,
        source_id: request.source_id,
        expected_head: request.expected_head,
        revision_id,
        occurrence_sequence,
        content_digest: request.content_digest,
        byte_length: request.byte_length,
        observed_at: request.observed_at.clone(),
        acquisition_kind: request.acquisition_kind,
        stability_receipt_ref: request.stability_receipt_ref.clone(),
        residency_closure: closure,
    };
    validate_preseal_profile(&profile)?;
    encode_source_revision_preseal_profile(&profile)?;

    control_port
        .retain_source_revision_preseal_profile(&profile, context)
        .map_err(|_| RegistryError::DurabilityRejected)?;

    Ok(SourceRevisionPreparation::Pending(
        SourceRevisionPendingPreparation {
            profile,
            replayed: false,
        },
    ))
}

/// Pure late binding from a retained profile to the final immutable occurrence.
///
/// This constructs the final SourceRevision from the exact original fields plus
/// the physical object-residency digest produced after AES. It performs no
/// durability work, deletes nothing and grants no authority: the retained
/// original stays readable, and the actual one-time commit remains the control
/// adapter compare-and-set.
///
/// # Errors
/// Returns record-invalid when the retained profile is not a valid descriptor.
pub fn late_bind_object_residency(
    profile: &SourceRevisionPresealProfile,
    object_residency_key_digest: ObjectResidencyKeyDigest,
) -> Result<SourceRevision, RegistryError> {
    validate_preseal_profile(profile)?;
    Ok(SourceRevision {
        revision_id: profile.revision_id,
        source_id: profile.source_id,
        occurrence_sequence: profile.occurrence_sequence,
        content_digest: profile.content_digest,
        byte_length: profile.byte_length,
        observed_at: profile.observed_at.clone(),
        acquisition_kind: profile.acquisition_kind,
        stability_receipt_ref: profile.stability_receipt_ref.clone(),
        object_residency_key_digest,
    })
}

/// Encodes one complete pre-seal profile through the canonical CBOR owner.
///
/// # Errors
/// Returns record-invalid when the profile is invalid or its complete canonical
/// encoding exceeds MAX_SOURCE_REVISION_PRESEAL_PROFILE_BYTES.
pub fn encode_source_revision_preseal_profile(
    profile: &SourceRevisionPresealProfile,
) -> Result<Vec<u8>, RegistryError> {
    validate_preseal_profile(profile)?;
    let value = preseal_profile_value(&profile.parts())
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let encoded =
        to_canonical_cbor(&value).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    if encoded.len() > MAX_SOURCE_REVISION_PRESEAL_PROFILE_BYTES {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    Ok(encoded.into_vec())
}

/// Decodes and canonicality-checks one complete pre-seal profile.
///
/// # Errors
/// Returns record-invalid for oversize bytes, non-canonical CBOR, an unknown or
/// missing field, a non-v4 occurrence UUID, a zero sequence or a non-canonical
/// residency closure.
pub fn decode_source_revision_preseal_profile(
    bytes: &[u8],
) -> Result<SourceRevisionPresealProfile, RegistryError> {
    if bytes.len() > MAX_SOURCE_REVISION_PRESEAL_PROFILE_BYTES {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    let value =
        parse_canonical_cbor(bytes).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let mut object = ClosedCanonicalObject::from_value(value, "source_revision_preseal_profile")
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;

    let header = take_text(&mut object, "header")?;
    if header != PRESEAL_PROFILE_HEADER {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    if take_u64(&mut object, "schema")? != PRESEAL_PROFILE_SCHEMA {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    let operation_id = OpaqueId::new(take_text(&mut object, "operation")?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let source_namespace_id =
        SourceNamespaceId::from_bytes(take_fixed::<16>(&mut object, "namespace")?);
    let source_id = SourceId::from_bytes(take_fixed::<16>(&mut object, "source")?);
    let expected_head =
        take_optional_uuid(&mut object, "expected_head")?.map(SourceRevisionId::from_bytes);
    let revision_id = SourceRevisionId::from_bytes(take_fixed::<16>(&mut object, "occurrence")?);
    let occurrence_sequence = take_u64(&mut object, "sequence")?;
    let content_digest = Blake3Digest32::from_bytes(take_fixed::<32>(&mut object, "content_digest")?);
    let byte_length = take_u64(&mut object, "byte_length")?;
    let observed_at = UtcTimestamp::parse(take_text(&mut object, "observed_at")?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let acquisition_kind = AcquisitionKind::parse(&take_text(&mut object, "acquisition")?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let stability_receipt_ref = ReceiptRef::new(take_text(&mut object, "stability_receipt")?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let residency_closure = take_closure(&mut object, "residency_closure")?;
    object
        .finish()
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;

    let profile = SourceRevisionPresealProfile {
        operation_id,
        source_namespace_id,
        source_id,
        expected_head,
        revision_id,
        occurrence_sequence,
        content_digest,
        byte_length,
        observed_at,
        acquisition_kind,
        stability_receipt_ref,
        residency_closure,
    };
    validate_preseal_profile(&profile)?;
    if encode_source_revision_preseal_profile(&profile)? != bytes {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    Ok(profile)
}

/// Borrowed field view used by the single canonical value builder.
#[derive(Clone, Copy)]
struct PresealProfileParts<'a> {
    operation_id: &'a OpaqueId,
    source_namespace_id: SourceNamespaceId,
    source_id: SourceId,
    expected_head: Option<SourceRevisionId>,
    revision_id: SourceRevisionId,
    occurrence_sequence: u64,
    content_digest: Blake3Digest32,
    byte_length: u64,
    observed_at: &'a UtcTimestamp,
    acquisition_kind: AcquisitionKind,
    stability_receipt_ref: &'a ReceiptRef,
    residency_closure: &'a OpaqueCanonicalBytes,
}

impl SourceRevisionPresealProfile {
    /// Borrowed field view of this profile.
    const fn parts(&self) -> PresealProfileParts<'_> {
        PresealProfileParts {
            operation_id: &self.operation_id,
            source_namespace_id: self.source_namespace_id,
            source_id: self.source_id,
            expected_head: self.expected_head,
            revision_id: self.revision_id,
            occurrence_sequence: self.occurrence_sequence,
            content_digest: self.content_digest,
            byte_length: self.byte_length,
            observed_at: &self.observed_at,
            acquisition_kind: self.acquisition_kind,
            stability_receipt_ref: &self.stability_receipt_ref,
            residency_closure: &self.residency_closure,
        }
    }
}

/// Builds the one canonical value for a pre-seal profile.
fn preseal_profile_value(parts: &PresealProfileParts<'_>) -> Result<CanonicalValue, ContractError> {
    let expected_head = match parts.expected_head {
        None => CanonicalValue::Null,
        Some(revision_id) => bytes_value(revision_id.as_bytes())?,
    };
    canonical_object(vec![
        ("header", text_value(PRESEAL_PROFILE_HEADER)?),
        ("schema", CanonicalValue::U64(PRESEAL_PROFILE_SCHEMA)),
        ("operation", text_value(parts.operation_id.as_str())?),
        (
            "namespace",
            bytes_value(parts.source_namespace_id.as_bytes())?,
        ),
        ("source", bytes_value(parts.source_id.as_bytes())?),
        ("expected_head", expected_head),
        ("occurrence", bytes_value(parts.revision_id.as_bytes())?),
        ("sequence", CanonicalValue::U64(parts.occurrence_sequence)),
        (
            "content_digest",
            bytes_value(parts.content_digest.as_bytes())?,
        ),
        ("byte_length", CanonicalValue::U64(parts.byte_length)),
        ("observed_at", text_value(parts.observed_at.as_str())?),
        ("acquisition", text_value(parts.acquisition_kind.as_str())?),
        (
            "stability_receipt",
            text_value(parts.stability_receipt_ref.as_str())?,
        ),
        (
            "residency_closure",
            bytes_value(parts.residency_closure.as_slice())?,
        ),
    ])
}

/// Enforces the complete profile bound before any identity is drawn.
///
/// The occurrence UUID is not yet known here, but every candidate UUID encodes
/// as a 16-byte CBOR byte string, so a zero placeholder has the exact width of
/// the real value. The sequence uses its widest possible encoding so this check
/// can never under-count. The real profile is re-checked when it is encoded.
fn check_preseal_profile_bound(
    request: &SourceRevisionPresealRequest,
    closure: &OpaqueCanonicalBytes,
) -> Result<(), RegistryError> {
    let parts = PresealProfileParts {
        operation_id: &request.operation_id,
        source_namespace_id: request.source_namespace_id,
        source_id: request.source_id,
        expected_head: request.expected_head,
        revision_id: SourceRevisionId::from_bytes([0_u8; 16]),
        occurrence_sequence: u64::MAX,
        content_digest: request.content_digest,
        byte_length: request.byte_length,
        observed_at: &request.observed_at,
        acquisition_kind: request.acquisition_kind,
        stability_receipt_ref: &request.stability_receipt_ref,
        residency_closure: closure,
    };
    let value =
        preseal_profile_value(&parts).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let encoded =
        to_canonical_cbor(&value).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    if encoded.len() > MAX_SOURCE_REVISION_PRESEAL_PROFILE_BYTES {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    Ok(())
}

/// Validates one retained pre-seal descriptor.
fn validate_preseal_profile(profile: &SourceRevisionPresealProfile) -> Result<(), RegistryError> {
    if !is_fresh_uuid_v4(&profile.revision_id) {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    if profile.occurrence_sequence == 0 {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    canonical_residency_closure(profile.residency_closure.as_slice())?;
    Ok(())
}

/// Requires exact canonical CBOR residency closure bytes.
fn canonical_residency_closure(bytes: &[u8]) -> Result<OpaqueCanonicalBytes, RegistryError> {
    if bytes.is_empty() {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    let value =
        parse_canonical_cbor(bytes).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let encoded =
        to_canonical_cbor(&value).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    if encoded.as_slice() != bytes {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    OpaqueCanonicalBytes::from_validated(bytes.to_vec())
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)
}

/// Loads the exact retained pre-seal descriptor for one operation.
fn load_retained_profile<C, P>(
    control_port: &P,
    operation_id: &OpaqueId,
    context: &OperationContext<C>,
) -> Result<Option<SourceRevisionPresealProfile>, RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionPreparePort<Cancellation = C>,
{
    control_port
        .load_source_revision_preseal_profile(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)
}

/// Proves a staged replay matches the complete original request.
fn staged_profile_matches_request(
    profile: &SourceRevisionPresealProfile,
    request: &SourceRevisionPresealRequest,
    closure: &OpaqueCanonicalBytes,
) -> Result<bool, RegistryError> {
    validate_preseal_profile(profile)?;
    Ok(profile.operation_id == request.operation_id
        && profile.source_namespace_id == request.source_namespace_id
        && profile.source_id == request.source_id
        && profile.expected_head == request.expected_head
        && profile.content_digest == request.content_digest
        && profile.byte_length == request.byte_length
        && profile.observed_at == request.observed_at
        && profile.acquisition_kind == request.acquisition_kind
        && profile.stability_receipt_ref == request.stability_receipt_ref
        && profile.residency_closure.as_slice() == closure.as_slice())
}

/// Proves a committed replay from the retained original and the final readback.
///
/// The retained original must match the request on every original field,
/// including the operation identity, the reserved occurrence UUID and the
/// sequence, and must then agree with the committed readback on every field it
/// carries. The readback object residency digest has no pre-seal counterpart
/// and is intentionally not compared.
fn verify_committed_preparation(
    profile: &SourceRevisionPresealProfile,
    readback: &SourceRevisionMutationReadback,
    request: &SourceRevisionPresealRequest,
    closure: &OpaqueCanonicalBytes,
) -> Result<(), RegistryError> {
    if !staged_profile_matches_request(profile, request, closure)? {
        return Err(RegistryError::OperationConflict);
    }
    let revision = &readback.revision;
    if readback.operation_id != request.operation_id
        || readback.source_namespace_id != request.source_namespace_id
        || readback.source_id != request.source_id
        || readback.expected_head != request.expected_head
        || revision.revision_id != profile.revision_id
        || revision.source_id != profile.source_id
        || revision.occurrence_sequence != profile.occurrence_sequence
        || revision.content_digest != profile.content_digest
        || revision.byte_length != profile.byte_length
        || revision.observed_at != profile.observed_at
        || revision.acquisition_kind != profile.acquisition_kind
        || revision.stability_receipt_ref != profile.stability_receipt_ref
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    Ok(())
}

/// Builds one bounded canonical object from validated entries.
fn canonical_object(
    entries: Vec<(&'static str, CanonicalValue)>,
) -> Result<CanonicalValue, ContractError> {
    BoundedMap::<CanonicalKey, CanonicalValue, MAX_MAP_ENTRIES>::from_entries(
        entries
            .into_iter()
            .map(|(key, value)| CanonicalKey::new_non_empty(key).map(|key| (key, value)))
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map(CanonicalValue::Object)
}

/// Bounded canonical text value.
fn text_value(value: &str) -> Result<CanonicalValue, ContractError> {
    CanonicalText::new(value).map(CanonicalValue::Text)
}

/// Bounded canonical byte-string value.
fn bytes_value(value: &[u8]) -> Result<CanonicalValue, ContractError> {
    BoundedBytes::new(value.to_vec()).map(CanonicalValue::Bytes)
}

/// Removes one required text field.
fn take_text(
    object: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<String, RegistryError> {
    match object
        .take_required(field)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?
    {
        CanonicalValue::Text(value) => Ok(value.into_string()),
        _ => Err(RegistryError::SourceRevisionRecordInvalid),
    }
}

/// Removes one required unsigned field.
fn take_u64(object: &mut ClosedCanonicalObject, field: &'static str) -> Result<u64, RegistryError> {
    match object
        .take_required(field)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?
    {
        CanonicalValue::U64(value) => Ok(value),
        _ => Err(RegistryError::SourceRevisionRecordInvalid),
    }
}

/// Removes one required fixed-width byte field.
fn take_fixed<const N: usize>(
    object: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<[u8; N], RegistryError> {
    match object
        .take_required(field)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?
    {
        CanonicalValue::Bytes(value) => value
            .as_slice()
            .try_into()
            .map_err(|_| RegistryError::SourceRevisionRecordInvalid),
        _ => Err(RegistryError::SourceRevisionRecordInvalid),
    }
}

/// Removes one required nullable UUID field.
fn take_optional_uuid(
    object: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<Option<[u8; 16]>, RegistryError> {
    match object
        .take_required(field)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?
    {
        CanonicalValue::Null => Ok(None),
        CanonicalValue::Bytes(value) => Ok(Some(
            value
                .as_slice()
                .try_into()
                .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?,
        )),
        _ => Err(RegistryError::SourceRevisionRecordInvalid),
    }
}

/// Removes one required canonical residency closure field.
fn take_closure(
    object: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<OpaqueCanonicalBytes, RegistryError> {
    match object
        .take_required(field)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?
    {
        CanonicalValue::Bytes(value) => canonical_residency_closure(value.as_slice()),
        _ => Err(RegistryError::SourceRevisionRecordInvalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RegistryPortError;
    use crate::revision::{SourceRevisionHead, SourceRevisionMutation, SourceRevisionSourceState};
    use search_contracts::{OpaqueRef, RequestId};
    use search_ports::{
        FakeCancellation, MutationIdentity, Port, PortErrorKind, PortRetryability,
    };
    use std::collections::{BTreeMap, VecDeque};

    const NAMESPACE: [u8; 16] = [0x20; 16];
    const SOURCE: [u8; 16] = [0x21; 16];

    fn context() -> OperationContext<FakeCancellation> {
        OperationContext::new(
            RequestId::from_bytes([0x11; 16]),
            5_000,
            FakeCancellation::new(false),
            OpaqueRef::new("budget:preseal-test").expect("budget reference"),
        )
        .expect("valid context")
    }

    fn port_error() -> RegistryPortError {
        crate::error::registry_port_error(
            PortErrorKind::DependencyUnavailable,
            PortRetryability::AfterReadback,
            RegistryError::DurabilityRejected,
            None,
        )
    }

    fn uuid_v4(seed: u8) -> SourceRevisionId {
        let mut bytes = [seed; 16];
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        SourceRevisionId::from_bytes(bytes)
    }

    /// Canonical CBOR residency closure with two tagged domain bodies.
    fn closure(seed: u8) -> Vec<u8> {
        let value = canonical_object(vec![
            ("access", bytes_value(&[seed; 16]).expect("access body")),
            ("scope", bytes_value(&[seed; 16]).expect("scope body")),
        ])
        .expect("closure object");
        to_canonical_cbor(&value).expect("closure CBOR").into_vec()
    }

    /// Canonical CBOR closure whose encoded profile exceeds the 8192 ceiling.
    fn oversized_closure() -> Vec<u8> {
        let value = canonical_object(vec![(
            "scope",
            bytes_value(&[0x5a; 9_000]).expect("oversized body"),
        )])
        .expect("closure object");
        to_canonical_cbor(&value).expect("closure CBOR").into_vec()
    }

    fn request(operation: &str, closure_bytes: Vec<u8>) -> SourceRevisionPresealRequest {
        SourceRevisionPresealRequest::new(
            OpaqueId::new(operation).expect("operation identity"),
            SourceNamespaceId::from_bytes(NAMESPACE),
            SourceId::from_bytes(SOURCE),
            None,
            Blake3Digest32::from_bytes([0xa1; 32]),
            37,
            UtcTimestamp::parse("2026-10-03T12:00:00.000000Z").expect("UTC timestamp"),
            AcquisitionKind::Filesystem,
            ReceiptRef::new("stable-read:a1").expect("stability receipt"),
            closure_bytes,
        )
    }

    #[derive(Default)]
    struct TestIds {
        ids: VecDeque<SourceRevisionId>,
        issued: usize,
    }

    impl Port for TestIds {
        type Error = RegistryPortError;
        type Cancellation = FakeCancellation;
    }

    impl SourceRevisionIdPort for TestIds {
        fn fresh_source_revision_id(
            &mut self,
            _context: &OperationContext<Self::Cancellation>,
        ) -> Result<SourceRevisionId, Self::Error> {
            self.issued += 1;
            self.ids.pop_front().ok_or_else(port_error)
        }
    }

    #[derive(Default)]
    struct TestControl {
        heads: BTreeMap<(SourceNamespaceId, SourceId), SourceRevisionHead>,
        revisions: BTreeMap<(SourceNamespaceId, SourceId, SourceRevisionId), SourceRevision>,
        operations: BTreeMap<OpaqueId, SourceRevisionMutationReadback>,
        profiles: BTreeMap<OpaqueId, SourceRevisionPresealProfile>,
        retained_bytes: BTreeMap<OpaqueId, Vec<u8>>,
    }

    impl TestControl {
        fn admit(&mut self) {
            self.heads.insert(
                (SourceNamespaceId::from_bytes(NAMESPACE), SourceId::from_bytes(SOURCE)),
                SourceRevisionHead {
                    source_namespace_id: SourceNamespaceId::from_bytes(NAMESPACE),
                    source_id: SourceId::from_bytes(SOURCE),
                    state: SourceRevisionSourceState::Active,
                    revision: None,
                },
            );
        }

        /// Simulates the manager/C02 one-time object commit and final CAS.
        fn commit(&mut self, object_residency_key_digest: ObjectResidencyKeyDigest) {
            let profile = self
                .profiles
                .values()
                .next()
                .expect("retained profile")
                .clone();
            let revision =
                late_bind_object_residency(&profile, object_residency_key_digest).expect("bind");
            let readback = SourceRevisionMutationReadback {
                operation_id: profile.operation_id.clone(),
                request_digest: Blake3Digest32::from_bytes([0x77; 32]),
                source_namespace_id: profile.source_namespace_id,
                source_id: profile.source_id,
                expected_head: profile.expected_head,
                revision: revision.clone(),
                receipt: ReceiptRef::new("test:revision:committed").expect("receipt"),
            };
            self.revisions.insert(
                (
                    profile.source_namespace_id,
                    profile.source_id,
                    revision.revision_id,
                ),
                revision.clone(),
            );
            self.operations.insert(profile.operation_id.clone(), readback);
            self.heads.insert(
                (profile.source_namespace_id, profile.source_id),
                SourceRevisionHead {
                    source_namespace_id: profile.source_namespace_id,
                    source_id: profile.source_id,
                    state: SourceRevisionSourceState::Active,
                    revision: Some(revision),
                },
            );
        }
    }

    impl Port for TestControl {
        type Error = RegistryPortError;
        type Cancellation = FakeCancellation;
    }

    impl SourceRevisionControlPort for TestControl {
        fn load_source_revision_operation(
            &self,
            operation_id: &OpaqueId,
            _context: &OperationContext<Self::Cancellation>,
        ) -> Result<Option<SourceRevisionMutationReadback>, Self::Error> {
            Ok(self.operations.get(operation_id).cloned())
        }

        fn load_source_revision_head(
            &self,
            source_namespace_id: &SourceNamespaceId,
            source_id: &SourceId,
            _context: &OperationContext<Self::Cancellation>,
        ) -> Result<Option<SourceRevisionHead>, Self::Error> {
            Ok(self
                .heads
                .get(&(*source_namespace_id, *source_id))
                .cloned())
        }

        fn load_source_revision(
            &self,
            source_namespace_id: &SourceNamespaceId,
            source_id: &SourceId,
            revision_id: &SourceRevisionId,
            _context: &OperationContext<Self::Cancellation>,
        ) -> Result<Option<SourceRevision>, Self::Error> {
            Ok(self
                .revisions
                .get(&(*source_namespace_id, *source_id, *revision_id))
                .cloned())
        }

        fn compare_and_append_source_revision(
            &mut self,
            _mutation: &SourceRevisionMutation,
            _context: &OperationContext<Self::Cancellation>,
            _identity: &MutationIdentity,
        ) -> Result<(), Self::Error> {
            // The pre-seal path never commits; this adapter proves that.
            Err(port_error())
        }
    }

    impl SourceRevisionPreparePort for TestControl {
        fn retain_source_revision_preseal_profile(
            &mut self,
            profile: &SourceRevisionPresealProfile,
            _context: &OperationContext<Self::Cancellation>,
        ) -> Result<(), Self::Error> {
            let bytes = encode_source_revision_preseal_profile(profile).map_err(|_| port_error())?;
            if let Some(existing) = self.retained_bytes.get(&profile.operation_id) {
                return if existing == &bytes {
                    Ok(())
                } else {
                    Err(port_error())
                };
            }
            self.retained_bytes
                .insert(profile.operation_id.clone(), bytes);
            self.profiles
                .insert(profile.operation_id.clone(), profile.clone());
            Ok(())
        }

        fn load_source_revision_preseal_profile(
            &self,
            operation_id: &OpaqueId,
            _context: &OperationContext<Self::Cancellation>,
        ) -> Result<Option<SourceRevisionPresealProfile>, Self::Error> {
            Ok(self.profiles.get(operation_id).cloned())
        }
    }

    fn ids() -> TestIds {
        TestIds {
            ids: VecDeque::from([uuid_v4(1), uuid_v4(2)]),
            issued: 0,
        }
    }

    fn control() -> TestControl {
        let mut control = TestControl::default();
        control.admit();
        control
    }

    fn staged(
        request: &SourceRevisionPresealRequest,
        ids: &mut TestIds,
        control: &mut TestControl,
    ) -> SourceRevisionPendingPreparation {
        match prepare_source_revision(request, ids, control, &context()).expect("staging") {
            SourceRevisionPreparation::Pending(pending) => pending,
            SourceRevisionPreparation::Committed(_) => panic!("staging is not a committed replay"),
        }
    }

    #[test]
    fn staged_preparation_replays_without_entropy_draw() {
        let mut control = control();
        let mut ids = ids();
        let request = request("preseal-op:pending", closure(0x31));

        let first = staged(&request, &mut ids, &mut control);
        assert!(!first.replayed);
        assert_eq!(first.profile.occurrence_sequence, 1);
        assert_eq!(ids.issued, 1);

        let second = staged(&request, &mut ids, &mut control);
        assert!(second.replayed);
        assert_eq!(second.profile, first.profile);
        assert_eq!(
            ids.issued, 1,
            "a staged replay must not draw another occurrence identity"
        );
    }

    #[test]
    fn committed_operation_replays_retained_original_without_entropy_draw() {
        let mut control = control();
        let mut ids = ids();
        let request = request("preseal-op:committed", closure(0x32));
        let original = staged(&request, &mut ids, &mut control).profile;

        let object_digest = ObjectResidencyKeyDigest::from_bytes([0x99; 32]);
        control.commit(object_digest);

        let replayed = match prepare_source_revision(&request, &mut ids, &mut control, &context())
            .expect("committed replay")
        {
            SourceRevisionPreparation::Committed(committed) => committed,
            SourceRevisionPreparation::Pending(_) => {
                panic!("a committed operation must resolve as a committed replay")
            }
        };
        assert_eq!(replayed.profile, original);
        assert_eq!(replayed.profile.revision_id, original.revision_id);
        assert_eq!(
            replayed.profile.occurrence_sequence,
            original.occurrence_sequence
        );
        assert_eq!(replayed.profile.operation_id, original.operation_id);
        assert_eq!(
            replayed.readback.revision.revision_id,
            original.revision_id
        );
        assert_eq!(
            replayed.readback.revision.occurrence_sequence,
            original.occurrence_sequence
        );
        assert_eq!(
            replayed.readback.revision.object_residency_key_digest,
            object_digest
        );
        assert_eq!(
            ids.issued, 1,
            "a committed replay must not draw another occurrence identity"
        );
    }

    #[test]
    fn committed_readback_without_retained_original_fails_closed() {
        let mut control = control();
        let mut ids = ids();
        let request = request("preseal-op:legacy", closure(0x33));
        staged(&request, &mut ids, &mut control);
        control.commit(ObjectResidencyKeyDigest::from_bytes([0x9a; 32]));

        // The legacy final readback alone must never be relabelled as proof.
        control.profiles.clear();
        assert_eq!(
            prepare_source_revision(&request, &mut ids, &mut control, &context())
                .expect_err("a readback without the retained original is not proof"),
            RegistryError::MutationOutcomeUnknown
        );
        assert_eq!(ids.issued, 1);
    }

    #[test]
    fn committed_readback_with_mismatched_retained_original_fails_closed() {
        let mut control = control();
        let mut ids = ids();
        let request = request("preseal-op:divergent", closure(0x37));
        let original = staged(&request, &mut ids, &mut control).profile;
        control.commit(ObjectResidencyKeyDigest::from_bytes([0x9b; 32]));

        // The retained original no longer names the committed occurrence.
        let mut tampered = original.clone();
        tampered.revision_id = uuid_v4(0x7c);
        control
            .profiles
            .insert(request.operation_id.clone(), tampered);
        assert_eq!(
            prepare_source_revision(&request, &mut ids, &mut control, &context())
                .expect_err("a divergent original cannot prove the committed operation"),
            RegistryError::MutationOutcomeUnknown
        );

        // A sequence divergence is refused the same way.
        let mut sequence_tampered = original;
        sequence_tampered.occurrence_sequence = 9;
        control
            .profiles
            .insert(request.operation_id.clone(), sequence_tampered);
        assert_eq!(
            prepare_source_revision(&request, &mut ids, &mut control, &context())
                .expect_err("a divergent sequence cannot prove the committed operation"),
            RegistryError::MutationOutcomeUnknown
        );
        assert_eq!(ids.issued, 1);
    }

    #[test]
    fn full_request_mismatch_refuses_and_keeps_the_retained_original() {
        let mut control = control();
        let mut ids = ids();
        let request = request("preseal-op:mismatch", closure(0x34));
        let original = staged(&request, &mut ids, &mut control).profile;

        let mut changed_closure = request.clone();
        changed_closure.residency_closure = closure(0x35);
        assert_eq!(
            prepare_source_revision(&changed_closure, &mut ids, &mut control, &context())
                .expect_err("another closure cannot reuse the operation identity"),
            RegistryError::OperationConflict
        );

        let mut changed_length = request.clone();
        changed_length.byte_length += 1;
        assert_eq!(
            prepare_source_revision(&changed_length, &mut ids, &mut control, &context())
                .expect_err("another length cannot reuse the operation identity"),
            RegistryError::OperationConflict
        );
        assert_eq!(ids.issued, 1);
        assert_eq!(
            control
                .load_source_revision_preseal_profile(&request.operation_id, &context())
                .expect("load")
                .expect("retained original survives"),
            original
        );
    }

    #[test]
    fn non_canonical_or_oversized_closure_refuses_before_identity_draw() {
        let mut control = control();
        let mut ids = ids();

        let mut non_canonical = request("preseal-op:non-canonical", closure(0x38));
        non_canonical.residency_closure = vec![0x9f, 0x01, 0xff];
        assert_eq!(
            prepare_source_revision(&non_canonical, &mut ids, &mut control, &context())
                .expect_err("indefinite-length CBOR is not canonical residency"),
            RegistryError::SourceRevisionRecordInvalid
        );

        let empty = request("preseal-op:empty", Vec::new());
        assert_eq!(
            prepare_source_revision(&empty, &mut ids, &mut control, &context())
                .expect_err("an absent residency closure cannot be staged"),
            RegistryError::SourceRevisionRecordInvalid
        );

        let oversized = request("preseal-op:oversized", oversized_closure());
        assert_eq!(
            prepare_source_revision(&oversized, &mut ids, &mut control, &context())
                .expect_err("a profile beyond the 8192 ceiling cannot be staged"),
            RegistryError::SourceRevisionRecordInvalid
        );
        assert_eq!(
            ids.issued, 0,
            "refused input must not consume an occurrence identity"
        );
        assert!(control.profiles.is_empty());
    }

    #[test]
    fn preseal_profile_codec_is_canonical_and_bounded() {
        let mut control = control();
        let mut ids = ids();
        let request = request("preseal-op:codec", closure(0x36));
        let original = staged(&request, &mut ids, &mut control).profile;

        let bytes = encode_source_revision_preseal_profile(&original).expect("encode");
        assert!(bytes.len() <= MAX_SOURCE_REVISION_PRESEAL_PROFILE_BYTES);
        assert_eq!(
            decode_source_revision_preseal_profile(&bytes).expect("decode"),
            original
        );

        let mut trailing = bytes;
        trailing.push(0xff);
        assert_eq!(
            decode_source_revision_preseal_profile(&trailing),
            Err(RegistryError::SourceRevisionRecordInvalid)
        );

        let mut zero_sequence = original.clone();
        zero_sequence.occurrence_sequence = 0;
        assert_eq!(
            encode_source_revision_preseal_profile(&zero_sequence),
            Err(RegistryError::SourceRevisionRecordInvalid)
        );

        let mut non_v4 = original;
        non_v4.revision_id = SourceRevisionId::from_bytes([0x00; 16]);
        assert_eq!(
            encode_source_revision_preseal_profile(&non_v4),
            Err(RegistryError::SourceRevisionRecordInvalid)
        );
    }
}


//! Typed source-revision occurrence registration and its vendor-neutral CAS boundary.
//!
//! This module registers metadata for an already accepted
//! `(SourceNamespaceId, SourceId)`. It does not read bytes, derive source
//! identity, admit a source, or retain revision payloads. The control adapter
//! owns durable source/head/operation state and must commit those values in one
//! transaction.

use search_contracts::{
    AcquisitionKind, Blake3Digest32, ObjectResidencyKeyDigest, OpaqueId, ReceiptRef, SourceId,
    SourceNamespaceId, SourceRevision, SourceRevisionId, UtcTimestamp,
};
use search_ports::{CancellationProbe, IdempotencyClass, MutationIdentity, OperationContext, Port};

use crate::error::{RegistryError, cancelled_before_commit};

const RECORD_MAGIC: &[u8; 4] = b"ESRV";
const READBACK_MAGIC: &[u8; 4] = b"ESRM";
const RECORD_VERSION: u8 = 1;
const REQUEST_DOMAIN: &[u8] = b"eliot-search/source-revision-registration/v1\0";
const UUID_V4_VERSION_MASK: u8 = 0xf0;
const UUID_V4_VERSION: u8 = 0x40;
const UUID_VARIANT_MASK: u8 = 0xc0;
const UUID_RFC4122_VARIANT: u8 = 0x80;
const REVISION_RECORD_FIXED_BYTES: usize = 147;

/// Input evidence for registering one new source-revision occurrence.
///
/// The operation identity and expected typed head bind retries and CAS. The
/// occurrence UUID and sequence are intentionally absent: the owner obtains a
/// fresh UUID through [`SourceRevisionIdPort`] and derives the sequence from
/// the exact durable head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionRegistrationRequest {
    /// Stable idempotency identity supplied by the operation owner.
    pub operation_id: OpaqueId,
    /// Namespace that durably admitted the source.
    pub source_namespace_id: SourceNamespaceId,
    /// Previously accepted source identity; it is never re-derived here.
    pub source_id: SourceId,
    /// Exact expected current revision, or `None` for the first occurrence.
    pub expected_head: Option<SourceRevisionId>,
    /// Digest of the stable read or immutable object accepted by the caller.
    pub content_digest: Blake3Digest32,
    /// Exact byte length bound to `content_digest` by the caller's stable read.
    pub byte_length: u64,
    /// Trusted observation time supplied by the acquisition owner.
    pub observed_at: UtcTimestamp,
    /// Closed acquisition source class.
    pub acquisition_kind: AcquisitionKind,
    /// Content-free proof reference for the accepted stable read.
    pub stability_receipt_ref: ReceiptRef,
    /// Complete residency-key digest for the admitted immutable object.
    pub object_residency_key_digest: ObjectResidencyKeyDigest,
}

impl SourceRevisionRegistrationRequest {
    /// Creates one source-revision registration request.
    #[must_use]
    pub const fn new(
        operation_id: OpaqueId,
        source_namespace_id: SourceNamespaceId,
        source_id: SourceId,
        expected_head: Option<SourceRevisionId>,
        content_digest: Blake3Digest32,
        byte_length: u64,
        observed_at: UtcTimestamp,
        acquisition_kind: AcquisitionKind,
        stability_receipt_ref: ReceiptRef,
        object_residency_key_digest: ObjectResidencyKeyDigest,
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
            object_residency_key_digest,
        }
    }

    /// BLAKE3-256 fingerprint of the versioned canonical request fields.
    ///
    /// The operation ID is the lookup key and is not repeated in this digest.
    /// The expected head is included so a retry cannot silently move its CAS
    /// base.
    #[must_use]
    pub fn request_digest(&self) -> Blake3Digest32 {
        let bytes = self.canonical_bytes();
        Blake3Digest32::from_bytes(*blake3::hash(&bytes).as_bytes())
    }

    /// Versioned canonical bytes used for the request fingerprint.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(192 + self.stability_receipt_ref.as_str().len());
        bytes.extend_from_slice(REQUEST_DOMAIN);
        bytes.push(RECORD_VERSION);
        bytes.extend_from_slice(self.source_namespace_id.as_bytes());
        bytes.extend_from_slice(self.source_id.as_bytes());
        append_optional_uuid(&mut bytes, self.expected_head.as_ref());
        bytes.extend_from_slice(self.content_digest.as_bytes());
        bytes.extend_from_slice(&self.byte_length.to_be_bytes());
        bytes.extend_from_slice(self.observed_at.as_str().as_bytes());
        bytes.push(acquisition_tag(self.acquisition_kind));
        let stability_receipt = self.stability_receipt_ref.as_str().as_bytes();
        bytes.extend_from_slice(&(stability_receipt.len() as u64).to_be_bytes());
        bytes.extend_from_slice(stability_receipt);
        bytes.extend_from_slice(self.object_residency_key_digest.as_bytes());
        bytes
    }
}

/// Source state observed with its exact current revision head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionHead {
    /// Namespace that durably admitted this source.
    pub source_namespace_id: SourceNamespaceId,
    /// Accepted source identity associated with this head.
    pub source_id: SourceId,
    /// Authoritative lifecycle state from the control store.
    pub state: SourceRevisionSourceState,
    /// Current immutable occurrence, absent before the first registration.
    pub revision: Option<SourceRevision>,
}

/// Lifecycle state returned from the authoritative source registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceRevisionSourceState {
    /// Source is admitted and permits new revision occurrences.
    Active,
    /// Source remains readable but cannot accept new revision occurrences.
    Retired,
}

/// One compare-and-append request for the control adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionMutation {
    /// Stable idempotency identity.
    pub operation_id: OpaqueId,
    /// Fingerprint of the complete caller request, excluding minted fields.
    pub request_digest: Blake3Digest32,
    /// Namespace that durably admitted the source.
    pub source_namespace_id: SourceNamespaceId,
    /// Accepted source identity key.
    pub source_id: SourceId,
    /// Head that must still be current in the atomic transaction.
    pub expected_head: Option<SourceRevisionId>,
    /// Complete typed immutable occurrence to retain.
    pub revision: SourceRevision,
}

/// Exact durable operation readback retained by the control adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionMutationReadback {
    /// Stable idempotency identity.
    pub operation_id: OpaqueId,
    /// Fingerprint of the complete caller request, excluding minted fields.
    pub request_digest: Blake3Digest32,
    /// Namespace that durably admitted the source.
    pub source_namespace_id: SourceNamespaceId,
    /// Accepted source identity key.
    pub source_id: SourceId,
    /// Head that was current before this committed occurrence.
    pub expected_head: Option<SourceRevisionId>,
    /// Complete typed immutable occurrence retained by the adapter.
    pub revision: SourceRevision,
    /// Content-free receipt created and persisted by the adapter.
    pub receipt: ReceiptRef,
}

/// Result of registering one immutable source-revision occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionRegistrationReceipt {
    /// Stable operation identity.
    pub operation_id: OpaqueId,
    /// Exact request fingerprint persisted with the operation.
    pub request_digest: Blake3Digest32,
    /// Namespace that durably admitted the source.
    pub source_namespace_id: SourceNamespaceId,
    /// Accepted source identity key.
    pub source_id: SourceId,
    /// Immutable occurrence committed by this operation.
    pub revision: SourceRevision,
    /// Current durable head observed during final verification.
    pub current_head: SourceRevision,
    /// Whether the operation's occurrence is still the current head.
    pub currentness: SourceRevisionCurrentness,
    /// Content-free receipt recovered from the durable operation record.
    pub receipt: ReceiptRef,
    /// Whether the result came from an earlier durable operation record.
    pub replayed: bool,
}

/// Currentness of the operation's immutable revision at exact readback time.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceRevisionCurrentness {
    /// The operation's occurrence is still the source's current head.
    Current,
    /// A later committed occurrence is now the source's current head.
    Superseded,
}

/// Qualified fresh-identity capability for new source-revision occurrences.
///
/// Production implementations must format bytes obtained from a qualified
/// operating-system CSPRNG as RFC 4122 version-4 UUIDs. They must not derive an
/// ID from source bytes, path text, the source ID, operation ID, or sequence.
pub trait SourceRevisionIdPort: Port {
    /// Obtains one fresh RFC 4122 version-4 occurrence identity.
    fn fresh_source_revision_id(
        &mut self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<SourceRevisionId, Self::Error>;
}

/// Atomic source-revision state boundary implemented by the control adapter.
///
/// `compare_and_append_source_revision` must check the exact durable active
/// admission row for `(source_namespace_id, source_id)`, compare the expected
/// head under that same key, reject any reused occurrence UUID, and atomically
/// retain the complete typed record, advance the per-source head, and bind the
/// stable operation ID and request digest. It returns `Ok(())` only when this
/// call committed the supplied occurrence. If the operation was committed by a
/// concurrent or earlier call, it returns an error so the caller recovers the
/// original occurrence through exact operation readback instead of adopting a
/// newly minted competing UUID.
/// These read methods return exact strongly consistent durable state. They
/// retain the operation binding so an unknown commit can be recovered without
/// minting another revision ID.
pub trait SourceRevisionControlPort: Port {
    /// Loads one operation binding by exact stable operation identity.
    fn load_source_revision_operation(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevisionMutationReadback>, Self::Error>;

    /// Loads admitted-source lifecycle and its complete current occurrence.
    ///
    /// `None` means that the source is not admitted. `Some` with no revision
    /// means it is admitted but has no occurrence yet.
    fn load_source_revision_head(
        &self,
        source_namespace_id: &SourceNamespaceId,
        source_id: &SourceId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevisionHead>, Self::Error>;

    /// Loads one complete immutable occurrence by its typed revision ID.
    fn load_source_revision(
        &self,
        source_namespace_id: &SourceNamespaceId,
        source_id: &SourceId,
        revision_id: &SourceRevisionId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevision>, Self::Error>;

    /// Atomically appends the immutable record and advances the expected head.
    fn compare_and_append_source_revision(
        &mut self,
        mutation: &SourceRevisionMutation,
        context: &OperationContext<Self::Cancellation>,
        identity: &MutationIdentity,
    ) -> Result<(), Self::Error>;
}

/// Registers one typed revision occurrence and verifies the exact durable outcome.
///
/// The request's typed `(SourceNamespaceId, SourceId)` is accepted input. This
/// function never reads source bytes or derives identity. It will not mint an ID on an exact
/// idempotent replay, and success always includes full operation, revision and
/// current-head readback.
pub fn register_source_revision<C, I, P>(
    request: &SourceRevisionRegistrationRequest,
    id_port: &mut I,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceRevisionRegistrationReceipt, RegistryError>
where
    C: CancellationProbe,
    I: SourceRevisionIdPort<Cancellation = C>,
    P: SourceRevisionControlPort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    let request_digest = request.request_digest();
    if let Some(readback) = load_operation(control_port, &request.operation_id, context)? {
        return verify_readback_for_request(
            request,
            request_digest,
            readback,
            control_port,
            context,
            true,
        );
    }

    let head = load_head(
        control_port,
        &request.source_namespace_id,
        &request.source_id,
        context,
    )?
    .ok_or(RegistryError::SourceNotFound)?;
    let previous_revision = validate_expected_head(request, &head)?;
    if head.state != SourceRevisionSourceState::Active {
        return Err(RegistryError::SourceRetired);
    }
    if let Some(previous_revision) = previous_revision {
        let persisted = load_revision(
            control_port,
            &request.source_namespace_id,
            &request.source_id,
            &previous_revision.revision_id,
            context,
        )?
        .ok_or(RegistryError::MutationOutcomeUnknown)?;
        if persisted != *previous_revision {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
    }
    let occurrence_sequence = previous_revision.map_or(Ok(1), |revision| {
        revision
            .occurrence_sequence
            .checked_add(1)
            .ok_or(RegistryError::ContractExhausted)
    })?;

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

    let revision = SourceRevision {
        revision_id,
        source_id: request.source_id,
        occurrence_sequence,
        content_digest: request.content_digest,
        byte_length: request.byte_length,
        observed_at: request.observed_at.clone(),
        acquisition_kind: request.acquisition_kind,
        stability_receipt_ref: request.stability_receipt_ref.clone(),
        object_residency_key_digest: request.object_residency_key_digest,
    };
    validate_source_revision(&revision)?;
    let mutation = SourceRevisionMutation {
        operation_id: request.operation_id.clone(),
        request_digest,
        source_namespace_id: request.source_namespace_id,
        source_id: request.source_id,
        expected_head: request.expected_head,
        revision,
    };
    let identity = MutationIdentity::new(
        request.operation_id.clone(),
        IdempotencyClass::RetrySameIdentity,
    );
    let commit_result =
        control_port.compare_and_append_source_revision(&mutation, context, &identity);
    recover_commit_result(
        request,
        request_digest,
        mutation,
        commit_result.is_ok(),
        control_port,
        context,
    )
}

/// Encodes one complete typed source-revision record in a versioned binary form.
pub fn encode_source_revision_record(revision: &SourceRevision) -> Result<Vec<u8>, RegistryError> {
    validate_source_revision(revision)?;
    let receipt = revision.stability_receipt_ref.as_str().as_bytes();
    let receipt_len =
        u16::try_from(receipt.len()).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let mut bytes = Vec::with_capacity(REVISION_RECORD_FIXED_BYTES + receipt.len());
    bytes.extend_from_slice(RECORD_MAGIC);
    bytes.push(RECORD_VERSION);
    bytes.extend_from_slice(revision.revision_id.as_bytes());
    bytes.extend_from_slice(revision.source_id.as_bytes());
    bytes.extend_from_slice(&revision.occurrence_sequence.to_be_bytes());
    bytes.extend_from_slice(revision.content_digest.as_bytes());
    bytes.extend_from_slice(&revision.byte_length.to_be_bytes());
    bytes.extend_from_slice(revision.observed_at.as_str().as_bytes());
    bytes.push(acquisition_tag(revision.acquisition_kind));
    bytes.extend_from_slice(&receipt_len.to_be_bytes());
    bytes.extend_from_slice(receipt);
    bytes.extend_from_slice(revision.object_residency_key_digest.as_bytes());
    Ok(bytes)
}

/// Decodes and canonicality-checks one complete typed source-revision record.
pub fn decode_source_revision_record(bytes: &[u8]) -> Result<SourceRevision, RegistryError> {
    let mut reader = Reader::new(bytes);
    reader.expect(RECORD_MAGIC)?;
    reader.expect(&[RECORD_VERSION])?;
    let revision_id = SourceRevisionId::from_bytes(reader.array()?);
    let source_id = SourceId::from_bytes(reader.array()?);
    let occurrence_sequence = reader.u64()?;
    let content_digest = Blake3Digest32::from_bytes(reader.array()?);
    let byte_length = reader.u64()?;
    let observed_at = UtcTimestamp::parse(reader.text_fixed(27)?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let acquisition_kind = acquisition_from_tag(reader.u8()?)?;
    let stability_receipt_ref = ReceiptRef::new(reader.text_u16()?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let object_residency_key_digest = ObjectResidencyKeyDigest::from_bytes(reader.array()?);
    reader.finish()?;
    let revision = SourceRevision {
        revision_id,
        source_id,
        occurrence_sequence,
        content_digest,
        byte_length,
        observed_at,
        acquisition_kind,
        stability_receipt_ref,
        object_residency_key_digest,
    };
    validate_source_revision(&revision)?;
    if encode_source_revision_record(&revision)? != bytes {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    Ok(revision)
}

/// Encodes a full durable operation readback for the control adapter.
pub fn encode_source_revision_mutation_readback(
    readback: &SourceRevisionMutationReadback,
) -> Result<Vec<u8>, RegistryError> {
    validate_source_revision(&readback.revision)?;
    if readback.source_id != readback.revision.source_id {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    let revision = encode_source_revision_record(&readback.revision)?;
    let operation_id = readback.operation_id.as_str().as_bytes();
    let operation_id_len = u16::try_from(operation_id.len())
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let receipt = readback.receipt.as_str().as_bytes();
    let receipt_len =
        u16::try_from(receipt.len()).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let revision_len =
        u32::try_from(revision.len()).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let mut bytes = Vec::with_capacity(
        4 + 1 + 2 + operation_id.len() + 16 + 16 + 32 + 17 + 4 + revision.len() + 2 + receipt.len(),
    );
    bytes.extend_from_slice(READBACK_MAGIC);
    bytes.push(RECORD_VERSION);
    bytes.extend_from_slice(&operation_id_len.to_be_bytes());
    bytes.extend_from_slice(operation_id);
    bytes.extend_from_slice(readback.source_namespace_id.as_bytes());
    bytes.extend_from_slice(readback.source_id.as_bytes());
    bytes.extend_from_slice(readback.request_digest.as_bytes());
    append_optional_uuid(&mut bytes, readback.expected_head.as_ref());
    bytes.extend_from_slice(&revision_len.to_be_bytes());
    bytes.extend_from_slice(&revision);
    bytes.extend_from_slice(&receipt_len.to_be_bytes());
    bytes.extend_from_slice(receipt);
    Ok(bytes)
}

/// Decodes and canonicality-checks a full durable operation readback.
pub fn decode_source_revision_mutation_readback(
    bytes: &[u8],
) -> Result<SourceRevisionMutationReadback, RegistryError> {
    let mut reader = Reader::new(bytes);
    reader.expect(READBACK_MAGIC)?;
    reader.expect(&[RECORD_VERSION])?;
    let operation_id = OpaqueId::new(reader.text_u16()?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let source_namespace_id = SourceNamespaceId::from_bytes(reader.array()?);
    let source_id = SourceId::from_bytes(reader.array()?);
    let request_digest = Blake3Digest32::from_bytes(reader.array()?);
    let expected_head = match reader.u8()? {
        0 => None,
        1 => Some(SourceRevisionId::from_bytes(reader.array()?)),
        _ => return Err(RegistryError::SourceRevisionRecordInvalid),
    };
    let revision_len =
        usize::try_from(reader.u32()?).map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let revision = decode_source_revision_record(reader.take(revision_len)?)?;
    let receipt = ReceiptRef::new(reader.text_u16()?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    reader.finish()?;
    let readback = SourceRevisionMutationReadback {
        operation_id,
        request_digest,
        source_namespace_id,
        source_id,
        expected_head,
        revision,
        receipt,
    };
    if readback.source_id != readback.revision.source_id {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    if encode_source_revision_mutation_readback(&readback)? != bytes {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    Ok(readback)
}

fn recover_commit_result<C, P>(
    request: &SourceRevisionRegistrationRequest,
    request_digest: Blake3Digest32,
    mutation: SourceRevisionMutation,
    commit_reported_success: bool,
    control_port: &P,
    context: &OperationContext<C>,
) -> Result<SourceRevisionRegistrationReceipt, RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionControlPort<Cancellation = C>,
{
    let operation = load_operation(control_port, &request.operation_id, context)
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?;
    let Some(readback) = operation else {
        let head = load_head(
            control_port,
            &request.source_namespace_id,
            &request.source_id,
            context,
        )
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?;
        let existing_revision = load_revision(
            control_port,
            &request.source_namespace_id,
            &request.source_id,
            &mutation.revision.revision_id,
            context,
        )
        .map_err(|_| RegistryError::MutationOutcomeUnknown)?;
        if existing_revision.is_some() {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
        if head_matches_expected(head.as_ref(), request)? {
            return if commit_reported_success {
                Err(RegistryError::MutationOutcomeUnknown)
            } else {
                Err(RegistryError::DurabilityRejected)
            };
        }
        return if commit_reported_success {
            Err(RegistryError::MutationOutcomeUnknown)
        } else {
            Err(RegistryError::SourceRevisionConflict)
        };
    };
    if readback.operation_id != request.operation_id {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    if readback.request_digest != request_digest {
        return Err(RegistryError::OperationConflict);
    }
    if readback.expected_head != mutation.expected_head
        || readback.source_namespace_id != mutation.source_namespace_id
        || readback.source_id != mutation.source_id
        || !readback_matches_request(&readback, request, request_digest)
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    verify_readback_for_request(
        request,
        request_digest,
        readback,
        control_port,
        context,
        !commit_reported_success,
    )
}

fn verify_readback_for_request<C, P>(
    request: &SourceRevisionRegistrationRequest,
    request_digest: Blake3Digest32,
    readback: SourceRevisionMutationReadback,
    control_port: &P,
    context: &OperationContext<C>,
    replayed: bool,
) -> Result<SourceRevisionRegistrationReceipt, RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionControlPort<Cancellation = C>,
{
    if readback.operation_id != request.operation_id {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    if readback.request_digest != request_digest
        || readback.source_namespace_id != request.source_namespace_id
        || readback.source_id != request.source_id
        || readback.expected_head != request.expected_head
    {
        return Err(RegistryError::OperationConflict);
    }
    if !readback_matches_request(&readback, request, request_digest) {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    verify_parent_sequence(request, &readback.revision, control_port, context)?;
    let persisted = load_revision(
        control_port,
        &request.source_namespace_id,
        &request.source_id,
        &readback.revision.revision_id,
        context,
    )
    .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    .ok_or(RegistryError::MutationOutcomeUnknown)?;
    if persisted != readback.revision {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    let head = load_head(
        control_port,
        &request.source_namespace_id,
        &request.source_id,
        context,
    )
    .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    .ok_or(RegistryError::MutationOutcomeUnknown)?;
    let current_head = head.revision.ok_or(RegistryError::MutationOutcomeUnknown)?;
    if head.source_namespace_id != request.source_namespace_id
        || head.source_id != request.source_id
        || current_head.source_id != request.source_id
        || current_head.occurrence_sequence < readback.revision.occurrence_sequence
        || (current_head.occurrence_sequence == readback.revision.occurrence_sequence
            && current_head.revision_id != readback.revision.revision_id)
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    let persisted_head = load_revision(
        control_port,
        &request.source_namespace_id,
        &request.source_id,
        &current_head.revision_id,
        context,
    )
    .map_err(|_| RegistryError::MutationOutcomeUnknown)?
    .ok_or(RegistryError::MutationOutcomeUnknown)?;
    if persisted_head != current_head {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    Ok(SourceRevisionRegistrationReceipt {
        operation_id: readback.operation_id,
        request_digest: readback.request_digest,
        source_namespace_id: readback.source_namespace_id,
        source_id: readback.source_id,
        currentness: if current_head.revision_id == readback.revision.revision_id {
            SourceRevisionCurrentness::Current
        } else {
            SourceRevisionCurrentness::Superseded
        },
        revision: readback.revision,
        current_head,
        receipt: readback.receipt,
        replayed,
    })
}

fn readback_matches_request(
    readback: &SourceRevisionMutationReadback,
    request: &SourceRevisionRegistrationRequest,
    request_digest: Blake3Digest32,
) -> bool {
    let revision = &readback.revision;
    let sequence_valid = match request.expected_head {
        None => revision.occurrence_sequence == 1,
        Some(_) => revision.occurrence_sequence > 1,
    };
    readback.request_digest == request_digest
        && readback.source_namespace_id == request.source_namespace_id
        && readback.source_id == request.source_id
        && readback.expected_head == request.expected_head
        && revision.source_id == request.source_id
        && sequence_valid
        && revision.content_digest == request.content_digest
        && revision.byte_length == request.byte_length
        && revision.observed_at == request.observed_at
        && revision.acquisition_kind == request.acquisition_kind
        && revision.stability_receipt_ref == request.stability_receipt_ref
        && revision.object_residency_key_digest == request.object_residency_key_digest
        && is_fresh_uuid_v4(&revision.revision_id)
}

fn verify_parent_sequence<C, P>(
    request: &SourceRevisionRegistrationRequest,
    revision: &SourceRevision,
    control_port: &P,
    context: &OperationContext<C>,
) -> Result<(), RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionControlPort<Cancellation = C>,
{
    match request.expected_head {
        None if revision.occurrence_sequence == 1 => Ok(()),
        None => Err(RegistryError::MutationOutcomeUnknown),
        Some(expected_head) => {
            let predecessor = load_revision(
                control_port,
                &request.source_namespace_id,
                &request.source_id,
                &expected_head,
                context,
            )
            .map_err(|_| RegistryError::MutationOutcomeUnknown)?
            .ok_or(RegistryError::MutationOutcomeUnknown)?;
            if predecessor.revision_id != expected_head
                || predecessor.source_id != request.source_id
                || predecessor.occurrence_sequence.checked_add(1)
                    != Some(revision.occurrence_sequence)
            {
                return Err(RegistryError::MutationOutcomeUnknown);
            }
            Ok(())
        }
    }
}

fn validate_expected_head<'a>(
    request: &SourceRevisionRegistrationRequest,
    head: &'a SourceRevisionHead,
) -> Result<Option<&'a SourceRevision>, RegistryError> {
    if head.source_namespace_id != request.source_namespace_id
        || head.source_id != request.source_id
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    if let Some(revision) = head.revision.as_ref() {
        validate_source_revision(revision)?;
        if revision.source_id != request.source_id {
            return Err(RegistryError::MutationOutcomeUnknown);
        }
    }
    match (request.expected_head, head.revision.as_ref()) {
        (None, None) => Ok(None),
        (Some(expected), Some(current)) if expected == current.revision_id => Ok(Some(current)),
        _ => Err(RegistryError::SourceRevisionConflict),
    }
}

fn head_matches_expected(
    head: Option<&SourceRevisionHead>,
    request: &SourceRevisionRegistrationRequest,
) -> Result<bool, RegistryError> {
    let Some(head) = head else {
        return Ok(false);
    };
    if head.source_namespace_id != request.source_namespace_id
        || head.source_id != request.source_id
    {
        return Err(RegistryError::MutationOutcomeUnknown);
    }
    Ok(match (request.expected_head, head.revision.as_ref()) {
        (None, None) => true,
        (Some(expected), Some(current)) => expected == current.revision_id,
        _ => false,
    })
}

fn validate_source_revision(revision: &SourceRevision) -> Result<(), RegistryError> {
    if revision.occurrence_sequence == 0 {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    Ok(())
}

fn is_fresh_uuid_v4(revision_id: &SourceRevisionId) -> bool {
    let bytes = revision_id.as_bytes();
    bytes[6] & UUID_V4_VERSION_MASK == UUID_V4_VERSION
        && bytes[8] & UUID_VARIANT_MASK == UUID_RFC4122_VARIANT
}

fn acquisition_tag(value: AcquisitionKind) -> u8 {
    match value {
        AcquisitionKind::Filesystem => 1,
        AcquisitionKind::GitObject => 2,
        AcquisitionKind::Imported => 3,
        AcquisitionKind::AdmittedIdeSnapshot => 4,
    }
}

fn acquisition_from_tag(tag: u8) -> Result<AcquisitionKind, RegistryError> {
    match tag {
        1 => Ok(AcquisitionKind::Filesystem),
        2 => Ok(AcquisitionKind::GitObject),
        3 => Ok(AcquisitionKind::Imported),
        4 => Ok(AcquisitionKind::AdmittedIdeSnapshot),
        _ => Err(RegistryError::SourceRevisionRecordInvalid),
    }
}

fn append_optional_uuid(bytes: &mut Vec<u8>, value: Option<&SourceRevisionId>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(value.as_bytes());
        }
    }
}

fn load_operation<C, P>(
    control_port: &P,
    operation_id: &OpaqueId,
    context: &OperationContext<C>,
) -> Result<Option<SourceRevisionMutationReadback>, RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionControlPort<Cancellation = C>,
{
    control_port
        .load_source_revision_operation(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)
}

fn load_head<C, P>(
    control_port: &P,
    source_namespace_id: &SourceNamespaceId,
    source_id: &SourceId,
    context: &OperationContext<C>,
) -> Result<Option<SourceRevisionHead>, RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionControlPort<Cancellation = C>,
{
    control_port
        .load_source_revision_head(source_namespace_id, source_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)
}

fn load_revision<C, P>(
    control_port: &P,
    source_namespace_id: &SourceNamespaceId,
    source_id: &SourceId,
    revision_id: &SourceRevisionId,
    context: &OperationContext<C>,
) -> Result<Option<SourceRevision>, RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionControlPort<Cancellation = C>,
{
    control_port
        .load_source_revision(source_namespace_id, source_id, revision_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), RegistryError> {
        if self.take(expected.len())? != expected {
            return Err(RegistryError::SourceRevisionRecordInvalid);
        }
        Ok(())
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], RegistryError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(RegistryError::SourceRevisionRecordInvalid)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(RegistryError::SourceRevisionRecordInvalid)?;
        self.position = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RegistryError> {
        self.take(N)?
            .try_into()
            .map_err(|_| RegistryError::SourceRevisionRecordInvalid)
    }

    fn u8(&mut self) -> Result<u8, RegistryError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, RegistryError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RegistryError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, RegistryError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn text_fixed(&mut self, length: usize) -> Result<String, RegistryError> {
        String::from_utf8(self.take(length)?.to_vec())
            .map_err(|_| RegistryError::SourceRevisionRecordInvalid)
    }

    fn text_u16(&mut self) -> Result<String, RegistryError> {
        let length = usize::from(self.u16()?);
        self.text_fixed(length)
    }

    fn finish(self) -> Result<(), RegistryError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(RegistryError::SourceRevisionRecordInvalid)
        }
    }
}

//! Staged pre-seal occurrence reservation for source revisions.
//!
//! This module provides a durable, typed pending-intent record that persists
//! the complete occurrence metadata BEFORE the AES sealing step. The final
//! physical object-residency digest is unknown until AES completes and is
//! bound later; this module NEVER fills the old SourceRevision with a dummy
//! digest.

use search_contracts::{
    AcquisitionKind, Blake3Digest32, ObjectResidencyKeyDigest, OpaqueId, ReceiptRef, SourceId,
    SourceNamespaceId, SourceRevisionId, UtcTimestamp,
};
use search_ports::{CancellationProbe, OperationContext, Port};

use crate::error::{RegistryError, cancelled_before_commit};
use crate::revision::{
    SourceRevisionControlPort, SourceRevisionIdPort, SourceRevisionRegistrationRequest,
    SourceRevisionSourceState,
    acquisition_tag, acquisition_from_tag, append_optional_uuid, is_fresh_uuid_v4,
    load_head, load_operation, load_revision, validate_expected_head,
};

const PENDING_MAGIC: &[u8; 4] = b"ESRP";
const PENDING_VERSION: u8 = 1;

/// Bounded port for exact pending-intent and committed-operation resolution.
///
/// The control adapter implements this to durably store and retrieve
/// staged pending records. This is the ONLY authority for pending state;
/// metadata/decoder alone is not durable authority.
pub trait SourceRevisionPreparePort: Port {
    /// Persists one exact pending occurrence descriptor.
    ///
    /// Returns `Ok(())` only when this call durably stored the pending record.
    /// If the operation already has a committed record, returns an error.
    fn store_pending_revision(
        &mut self,
        pending: &SourceRevisionPendingDescriptor,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<(), Self::Error>;

    /// Loads the exact pending descriptor for one operation, if any.
    fn load_pending_revision(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevisionPendingDescriptor>, Self::Error>;

    /// Removes the exact pending descriptor for one operation.
    ///
    /// Called after the pending record has been resolved (either committed
    /// or explicitly abandoned). Returns `Ok(())` only when the pending
    /// record was durably removed or was already absent.
    fn clear_pending_revision(
        &mut self,
        operation_id: &OpaqueId,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<(), Self::Error>;
}

/// Complete typed pending occurrence descriptor persisted before AES sealing.
///
/// This captures the full residency closure: namespace, source, UUID,
/// sequence, expected head, content digest, byte length, observed-at,
/// stability receipt, and object-residency key digest. The final physical
/// object-residency digest is NOT included here; it is bound after AES.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionPendingDescriptor {
    /// Stable idempotency identity.
    pub operation_id: OpaqueId,
    /// Namespace that durably admitted the source.
    pub source_namespace_id: SourceNamespaceId,
    /// Accepted source identity.
    pub source_id: SourceId,
    /// Exact expected current revision, or `None` for the first occurrence.
    pub expected_head: Option<SourceRevisionId>,
    /// Fresh occurrence UUID reserved for this pending operation.
    pub revision_id: SourceRevisionId,
    /// Occurrence sequence derived from the exact durable head.
    pub occurrence_sequence: u64,
    /// Digest of the stable read or immutable object accepted by the caller.
    pub content_digest: Blake3Digest32,
    /// Exact byte length bound to `content_digest`.
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

/// Result of preparing (staging) one pending occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionPendingReceipt {
    /// The exact pending descriptor that was persisted.
    pub descriptor: SourceRevisionPendingDescriptor,
    /// Whether this was a replay of an existing pending record.
    pub replayed: bool,
}

/// Stages one pending occurrence descriptor before AES sealing.
///
/// This function:
/// 1. Checks for an existing committed operation (replay).
/// 2. Checks for an existing pending record (replay).
/// 3. Validates the active source and expected head.
/// 4. Mints a fresh UUID and derives the sequence.
/// 5. Persists the complete pending descriptor.
///
/// The final physical object-residency digest is NOT included; it is
/// bound after AES sealing completes.
pub fn prepare_source_revision_pending<C, I, P>(
    request: &SourceRevisionRegistrationRequest,
    id_port: &mut I,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceRevisionPendingReceipt, RegistryError>
where
    C: CancellationProbe,
    I: SourceRevisionIdPort<Cancellation = C>,
    P: SourceRevisionControlPort<Cancellation = C>
        + SourceRevisionPreparePort<Cancellation = C>,
{
    cancelled_before_commit(context)?;

    // Check for existing committed operation (replay)
    if load_operation(control_port, &request.operation_id, context)?.is_some() {
        return Err(RegistryError::OperationConflict);
    }

    // Check for existing pending record (replay)
    if let Some(existing) = control_port
        .load_pending_revision(&request.operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?
    {
        if pending_matches_request(&existing, request) {
            return Ok(SourceRevisionPendingReceipt {
                descriptor: existing,
                replayed: true,
            });
        }
        return Err(RegistryError::OperationConflict);
    }

    // Validate active source and expected head
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

    // Mint fresh UUID and derive sequence
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

    let descriptor = SourceRevisionPendingDescriptor {
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
        object_residency_key_digest: request.object_residency_key_digest,
    };

    control_port
        .store_pending_revision(&descriptor, context)
        .map_err(|_| RegistryError::DurabilityRejected)?;

    Ok(SourceRevisionPendingReceipt {
        descriptor,
        replayed: false,
    })
}

/// Recovers a pending occurrence descriptor for one operation.
///
/// This is used when the process restarts and needs to resolve a pending
/// operation. It loads the pending descriptor without clearing it.
pub fn recover_pending_revision<C, P>(
    operation_id: &OpaqueId,
    control_port: &P,
    context: &OperationContext<C>,
) -> Result<Option<SourceRevisionPendingDescriptor>, RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionControlPort<Cancellation = C>
        + SourceRevisionPreparePort<Cancellation = C>,
{
    cancelled_before_commit(context)?;
    control_port
        .load_pending_revision(operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)
}

/// Resolves a pending occurrence after AES sealing completes.
///
/// This function loads the exact pending descriptor, verifies it matches
/// the request, clears the pending record, and returns the descriptor.
pub fn resolve_pending_revision<C, P>(
    request: &SourceRevisionRegistrationRequest,
    control_port: &mut P,
    context: &OperationContext<C>,
) -> Result<SourceRevisionPendingDescriptor, RegistryError>
where
    C: CancellationProbe,
    P: SourceRevisionControlPort<Cancellation = C>
        + SourceRevisionPreparePort<Cancellation = C>,
{
    cancelled_before_commit(context)?;

    let descriptor = control_port
        .load_pending_revision(&request.operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?
        .ok_or(RegistryError::SourceNotFound)?;

    if !pending_matches_request(&descriptor, request) {
        return Err(RegistryError::OperationConflict);
    }

    control_port
        .clear_pending_revision(&request.operation_id, context)
        .map_err(|_| RegistryError::DurabilityRejected)?;

    Ok(descriptor)
}

fn pending_matches_request(
    descriptor: &SourceRevisionPendingDescriptor,
    request: &SourceRevisionRegistrationRequest,
) -> bool {
    descriptor.source_namespace_id == request.source_namespace_id
        && descriptor.source_id == request.source_id
        && descriptor.expected_head == request.expected_head
        && descriptor.content_digest == request.content_digest
        && descriptor.byte_length == request.byte_length
        && descriptor.observed_at == request.observed_at
        && descriptor.acquisition_kind == request.acquisition_kind
        && descriptor.stability_receipt_ref == request.stability_receipt_ref
        && descriptor.object_residency_key_digest == request.object_residency_key_digest
}

/// Encodes one complete pending descriptor in a versioned binary form.
pub fn encode_pending_descriptor(
    descriptor: &SourceRevisionPendingDescriptor,
) -> Result<Vec<u8>, RegistryError> {
    let operation_id = descriptor.operation_id.as_str().as_bytes();
    let operation_id_len = u16::try_from(operation_id.len())
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let stability_receipt = descriptor.stability_receipt_ref.as_str().as_bytes();
    let stability_receipt_len = u16::try_from(stability_receipt.len())
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;

    let mut bytes = Vec::with_capacity(
        4 + 1
            + 2
            + operation_id.len()
            + 16
            + 16
            + 1
            + 16
            + 16
            + 8
            + 32
            + 8
            + 27
            + 1
            + 2
            + stability_receipt.len()
            + 32,
    );
    bytes.extend_from_slice(PENDING_MAGIC);
    bytes.push(PENDING_VERSION);
    bytes.extend_from_slice(&operation_id_len.to_be_bytes());
    bytes.extend_from_slice(operation_id);
    bytes.extend_from_slice(descriptor.source_namespace_id.as_bytes());
    bytes.extend_from_slice(descriptor.source_id.as_bytes());
    append_optional_uuid(&mut bytes, descriptor.expected_head.as_ref());
    bytes.extend_from_slice(descriptor.revision_id.as_bytes());
    bytes.extend_from_slice(&descriptor.occurrence_sequence.to_be_bytes());
    bytes.extend_from_slice(descriptor.content_digest.as_bytes());
    bytes.extend_from_slice(&descriptor.byte_length.to_be_bytes());
    bytes.extend_from_slice(descriptor.observed_at.as_str().as_bytes());
    bytes.push(acquisition_tag(descriptor.acquisition_kind));
    bytes.extend_from_slice(&stability_receipt_len.to_be_bytes());
    bytes.extend_from_slice(stability_receipt);
    bytes.extend_from_slice(descriptor.object_residency_key_digest.as_bytes());
    Ok(bytes)
}

/// Decodes and canonicality-checks one complete pending descriptor.
pub fn decode_pending_descriptor(
    bytes: &[u8],
) -> Result<SourceRevisionPendingDescriptor, RegistryError> {
    let mut reader = Reader::new(bytes);
    reader.expect(PENDING_MAGIC)?;
    reader.expect(&[PENDING_VERSION])?;
    let operation_id = OpaqueId::new(reader.text_u16()?)
        .map_err(|_| RegistryError::SourceRevisionRecordInvalid)?;
    let source_namespace_id = SourceNamespaceId::from_bytes(reader.array()?);
    let source_id = SourceId::from_bytes(reader.array()?);
    let expected_head = match reader.u8()? {
        0 => None,
        1 => Some(SourceRevisionId::from_bytes(reader.array()?)),
        _ => return Err(RegistryError::SourceRevisionRecordInvalid),
    };
    let revision_id = SourceRevisionId::from_bytes(reader.array()?);
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

    let descriptor = SourceRevisionPendingDescriptor {
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
        object_residency_key_digest,
    };

    if !is_fresh_uuid_v4(&descriptor.revision_id) {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    if descriptor.occurrence_sequence == 0 {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    if encode_pending_descriptor(&descriptor)? != bytes {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    Ok(descriptor)
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

#[cfg(test)]
mod tests {
    use super::*;
    fn uuid_v4(seed: u8) -> SourceRevisionId {
        let mut bytes = [seed; 16];
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        SourceRevisionId::from_bytes(bytes)
    }

    #[test]
    fn pending_descriptor_codec_roundtrip_is_canonical() {
        let descriptor = SourceRevisionPendingDescriptor {
            operation_id: OpaqueId::new("op:test").expect("operation identity"),
            source_namespace_id: SourceNamespaceId::from_bytes([0x20; 16]),
            source_id: SourceId::from_bytes([0x21; 16]),
            expected_head: None,
            revision_id: uuid_v4(1),
            occurrence_sequence: 1,
            content_digest: Blake3Digest32::from_bytes([0xa1; 32]),
            byte_length: 37,
            observed_at: UtcTimestamp::parse("2026-10-03T12:00:00.000000Z").expect("UTC timestamp"),
            acquisition_kind: AcquisitionKind::Filesystem,
            stability_receipt_ref: ReceiptRef::new("stable-read:a1").expect("stability receipt"),
            object_residency_key_digest: ObjectResidencyKeyDigest::from_bytes([0x42; 32]),
        };
        let encoded = encode_pending_descriptor(&descriptor).expect("encode");
        let decoded = decode_pending_descriptor(&encoded).expect("decode");
        assert_eq!(decoded, descriptor);
    }

    #[test]
    fn pending_descriptor_rejects_non_v4_uuid() {
        let descriptor = SourceRevisionPendingDescriptor {
            operation_id: OpaqueId::new("op:test").expect("operation identity"),
            source_namespace_id: SourceNamespaceId::from_bytes([0x20; 16]),
            source_id: SourceId::from_bytes([0x21; 16]),
            expected_head: None,
            revision_id: SourceRevisionId::from_bytes([0x00; 16]),
            occurrence_sequence: 1,
            content_digest: Blake3Digest32::from_bytes([0xa1; 32]),
            byte_length: 37,
            observed_at: UtcTimestamp::parse("2026-10-03T12:00:00.000000Z").expect("UTC timestamp"),
            acquisition_kind: AcquisitionKind::Filesystem,
            stability_receipt_ref: ReceiptRef::new("stable-read:a1").expect("stability receipt"),
            object_residency_key_digest: ObjectResidencyKeyDigest::from_bytes([0x42; 32]),
        };
        let encoded = encode_pending_descriptor(&descriptor).expect("encode");
        assert!(decode_pending_descriptor(&encoded).is_err());
    }

    #[test]
    fn pending_descriptor_rejects_zero_sequence() {
        let descriptor = SourceRevisionPendingDescriptor {
            operation_id: OpaqueId::new("op:test").expect("operation identity"),
            source_namespace_id: SourceNamespaceId::from_bytes([0x20; 16]),
            source_id: SourceId::from_bytes([0x21; 16]),
            expected_head: None,
            revision_id: uuid_v4(1),
            occurrence_sequence: 0,
            content_digest: Blake3Digest32::from_bytes([0xa1; 32]),
            byte_length: 37,
            observed_at: UtcTimestamp::parse("2026-10-03T12:00:00.000000Z").expect("UTC timestamp"),
            acquisition_kind: AcquisitionKind::Filesystem,
            stability_receipt_ref: ReceiptRef::new("stable-read:a1").expect("stability receipt"),
            object_residency_key_digest: ObjectResidencyKeyDigest::from_bytes([0x42; 32]),
        };
        let encoded = encode_pending_descriptor(&descriptor).expect("encode");
        assert!(decode_pending_descriptor(&encoded).is_err());
    }

    #[test]
    fn pending_descriptor_rejects_trailing_bytes() {
        let descriptor = SourceRevisionPendingDescriptor {
            operation_id: OpaqueId::new("op:test").expect("operation identity"),
            source_namespace_id: SourceNamespaceId::from_bytes([0x20; 16]),
            source_id: SourceId::from_bytes([0x21; 16]),
            expected_head: None,
            revision_id: uuid_v4(1),
            occurrence_sequence: 1,
            content_digest: Blake3Digest32::from_bytes([0xa1; 32]),
            byte_length: 37,
            observed_at: UtcTimestamp::parse("2026-10-03T12:00:00.000000Z").expect("UTC timestamp"),
            acquisition_kind: AcquisitionKind::Filesystem,
            stability_receipt_ref: ReceiptRef::new("stable-read:a1").expect("stability receipt"),
            object_residency_key_digest: ObjectResidencyKeyDigest::from_bytes([0x42; 32]),
        };
        let mut encoded = encode_pending_descriptor(&descriptor).expect("encode");
        encoded.push(0xff);
        assert!(decode_pending_descriptor(&encoded).is_err());
    }
}

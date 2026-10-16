//! Staged pre-seal occurrence reservation for source revisions.
//!
//! This module provides a durable, typed pending-intent record that persists
//! the complete occurrence metadata BEFORE the AES sealing step. The final
//! physical object-residency digest is unknown until AES completes and is
//! bound later via a pure one-time late-binding function; this module NEVER
//! fills the old SourceRevision with a dummy digest.

use search_contracts::{
    AcquisitionKind, Blake3Digest32, ObjectResidencyKeyDigest, OpaqueId, ReceiptRef, SourceId,
    SourceNamespaceId, SourceRevision, SourceRevisionId, UtcTimestamp,
};
use search_ports::{CancellationProbe, OperationContext, Port};

use crate::error::{RegistryError, cancelled_before_commit};
use crate::revision::{
    SourceRevisionControlPort, SourceRevisionIdPort, SourceRevisionSourceState,
    acquisition_tag, acquisition_from_tag, append_optional_uuid, is_fresh_uuid_v4,
    load_head, load_operation, load_revision, validate_expected_head_parts,
};

const PENDING_MAGIC: &[u8; 4] = b"ESRP";
const PENDING_VERSION: u8 = 1;
const MAX_RESIDENCY_CLOSURE_BYTES: usize = 65_535;

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
}

/// Additive pre-seal request that excludes the final physical object digest.
///
/// This is the input to prepare_source_revision_pending. It carries the full
/// canonical residency closure as metadata bytes (not the final object digest,
/// which is unknown before AES). The old SourceRevisionRegistrationRequest is
/// unchanged and remains for the post-seal registration path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionPresealRequest {
    /// Stable idempotency identity.
    pub operation_id: OpaqueId,
    /// Namespace that durably admitted the source.
    pub source_namespace_id: SourceNamespaceId,
    /// Accepted source identity.
    pub source_id: SourceId,
    /// Exact expected current revision, or `None` for the first occurrence.
    pub expected_head: Option<SourceRevisionId>,
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
    /// Full bounded canonical residency closure bytes (metadata, not the
    /// final physical object-residency digest which is unknown before AES).
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

/// Complete typed pending occurrence descriptor persisted before AES sealing.
///
/// This captures the full residency closure: namespace, source, UUID,
/// sequence, expected head, content digest, byte length, observed-at,
/// stability receipt, and canonical residency closure bytes. The final
/// physical object-residency digest is NOT included here; it is bound
/// after AES via bind_pending_to_object.
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
    /// Full bounded canonical residency closure bytes (metadata).
    pub residency_closure: Vec<u8>,
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
/// 1. Checks for an existing committed operation (distinguishes replay vs conflict).
/// 2. Checks for an existing pending record (replay).
/// 3. Validates the active source and expected head.
/// 4. Mints a fresh UUID and derives the sequence.
/// 5. Persists the complete pending descriptor.
///
/// The final physical object-residency digest is NOT included; it is
/// bound after AES sealing completes via bind_pending_to_object.
pub fn prepare_source_revision_pending<C, I, P>(
    request: &SourceRevisionPresealRequest,
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

    if request.residency_closure.len() > MAX_RESIDENCY_CLOSURE_BYTES {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }

    // Check for existing committed operation (distinguish replay vs conflict)
    if let Some(readback) = load_operation(control_port, &request.operation_id, context)? {
        if committed_operation_matches_preseal(&readback, request) {
            // Exact committed replay: return the existing committed result.
            // The caller should use the committed readback directly.
            return Err(RegistryError::OperationConflict);
        }
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
    let previous_revision = validate_expected_head_parts(
        request.source_namespace_id,
        request.source_id,
        request.expected_head,
        &head,
    )?;
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
        residency_closure: request.residency_closure.clone(),
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

/// Pure one-time late binding from a pending descriptor to the final immutable
/// SourceRevision with the physical object-residency digest.
///
/// This function does NOT delete the pending record. The control adapter
/// atomically resolves the pending record with the final commit. The pending
/// record survives until the control adapter completes the atomic commit.
///
/// Returns the final immutable SourceRevision with the exact original
/// UUID, sequence, observed-at, and all original metadata, plus the final
/// object-residency digest bound after AES.
pub fn bind_pending_to_object(
    pending: &SourceRevisionPendingDescriptor,
    object_residency_key_digest: ObjectResidencyKeyDigest,
) -> Result<SourceRevision, RegistryError> {
    if !is_fresh_uuid_v4(&pending.revision_id) {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    if pending.occurrence_sequence == 0 {
        return Err(RegistryError::SourceRevisionRecordInvalid);
    }
    Ok(SourceRevision {
        revision_id: pending.revision_id,
        source_id: pending.source_id,
        occurrence_sequence: pending.occurrence_sequence,
        content_digest: pending.content_digest,
        byte_length: pending.byte_length,
        observed_at: pending.observed_at.clone(),
        acquisition_kind: pending.acquisition_kind,
        stability_receipt_ref: pending.stability_receipt_ref.clone(),
        object_residency_key_digest,
    })
}

fn committed_operation_matches_preseal(
    readback: &crate::revision::SourceRevisionMutationReadback,
    request: &SourceRevisionPresealRequest,
) -> bool {
    readback.source_namespace_id == request.source_namespace_id
        && readback.source_id == request.source_id
        && readback.expected_head == request.expected_head
        && readback.revision.source_id == request.source_id
        && readback.revision.content_digest == request.content_digest
        && readback.revision.byte_length == request.byte_length
        && readback.revision.observed_at == request.observed_at
        && readback.revision.acquisition_kind == request.acquisition_kind
        && readback.revision.stability_receipt_ref == request.stability_receipt_ref
}

fn pending_matches_request(
    descriptor: &SourceRevisionPendingDescriptor,
    request: &SourceRevisionPresealRequest,
) -> bool {
    descriptor.source_namespace_id == request.source_namespace_id
        && descriptor.source_id == request.source_id
        && descriptor.expected_head == request.expected_head
        && descriptor.content_digest == request.content_digest
        && descriptor.byte_length == request.byte_length
        && descriptor.observed_at == request.observed_at
        && descriptor.acquisition_kind == request.acquisition_kind
        && descriptor.stability_receipt_ref == request.stability_receipt_ref
        && descriptor.residency_closure == request.residency_closure
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
    let residency_closure_len = u16::try_from(descriptor.residency_closure.len())
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
            + 2
            + descriptor.residency_closure.len(),
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
    bytes.extend_from_slice(&residency_closure_len.to_be_bytes());
    bytes.extend_from_slice(&descriptor.residency_closure);
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
    let residency_closure_len = usize::from(reader.u16()?);
    let residency_closure = reader.take(residency_closure_len)?.to_vec();
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
        residency_closure,
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
    use search_ports::{FakeCancellation, OperationContext};
    use crate::error::RegistryPortError;
    use std::collections::BTreeMap;

    fn context() -> OperationContext<FakeCancellation> {
        OperationContext::new(
            search_contracts::RequestId::from_bytes([0x11; 16]),
            5_000,
            FakeCancellation::new(false),
            search_contracts::OpaqueRef::new("budget:prepare-test").expect("budget reference"),
        )
        .expect("valid context")
    }

    fn uuid_v4(seed: u8) -> SourceRevisionId {
        let mut bytes = [seed; 16];
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        SourceRevisionId::from_bytes(bytes)
    }

    fn preseal_request(operation: &str) -> SourceRevisionPresealRequest {
        SourceRevisionPresealRequest::new(
            OpaqueId::new(operation).expect("operation identity"),
            SourceNamespaceId::from_bytes([0x20; 16]),
            SourceId::from_bytes([0x21; 16]),
            None,
            Blake3Digest32::from_bytes([0xa1; 32]),
            37,
            UtcTimestamp::parse("2026-10-03T12:00:00.000000Z").expect("UTC timestamp"),
            AcquisitionKind::Filesystem,
            ReceiptRef::new("stable-read:a1").expect("stability receipt"),
            vec![0x42, 0x43, 0x44],
        )
    }

    fn pending_descriptor(operation: &str) -> SourceRevisionPendingDescriptor {
        SourceRevisionPendingDescriptor {
            operation_id: OpaqueId::new(operation).expect("operation identity"),
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
            residency_closure: vec![0x42, 0x43, 0x44],
        }
    }

    #[derive(Default)]
    struct TestPreparePort {
        pending: BTreeMap<OpaqueId, SourceRevisionPendingDescriptor>,
    }

    impl Port for TestPreparePort {
        type Error = RegistryPortError;
        type Cancellation = FakeCancellation;
    }

    impl SourceRevisionPreparePort for TestPreparePort {
        fn store_pending_revision(
            &mut self,
            pending: &SourceRevisionPendingDescriptor,
            _context: &OperationContext<Self::Cancellation>,
        ) -> Result<(), Self::Error> {
            self.pending
                .insert(pending.operation_id.clone(), pending.clone());
            Ok(())
        }

        fn load_pending_revision(
            &self,
            operation_id: &OpaqueId,
            _context: &OperationContext<Self::Cancellation>,
        ) -> Result<Option<SourceRevisionPendingDescriptor>, Self::Error> {
            Ok(self.pending.get(operation_id).cloned())
        }
    }

    #[test]
    fn preseal_request_excludes_final_object_digest() {
        let request = preseal_request("op:test");
        assert_eq!(request.residency_closure, vec![0x42, 0x43, 0x44]);
    }

    #[test]
    fn pending_descriptor_codec_roundtrip_is_canonical() {
        let descriptor = pending_descriptor("op:test");
        let encoded = encode_pending_descriptor(&descriptor).expect("encode");
        let decoded = decode_pending_descriptor(&encoded).expect("decode");
        assert_eq!(decoded, descriptor);
    }

    #[test]
    fn pending_descriptor_rejects_non_v4_uuid() {
        let mut descriptor = pending_descriptor("op:test");
        descriptor.revision_id = SourceRevisionId::from_bytes([0x00; 16]);
        let encoded = encode_pending_descriptor(&descriptor).expect("encode");
        assert!(decode_pending_descriptor(&encoded).is_err());
    }

    #[test]
    fn pending_descriptor_rejects_zero_sequence() {
        let mut descriptor = pending_descriptor("op:test");
        descriptor.occurrence_sequence = 0;
        let encoded = encode_pending_descriptor(&descriptor).expect("encode");
        assert!(decode_pending_descriptor(&encoded).is_err());
    }

    #[test]
    fn pending_descriptor_rejects_trailing_bytes() {
        let descriptor = pending_descriptor("op:test");
        let mut encoded = encode_pending_descriptor(&descriptor).expect("encode");
        encoded.push(0xff);
        assert!(decode_pending_descriptor(&encoded).is_err());
    }

    #[test]
    fn bind_pending_to_object_preserves_original_metadata() {
        let pending = pending_descriptor("op:test");
        let final_digest = ObjectResidencyKeyDigest::from_bytes([0x99; 32]);
        let revision = bind_pending_to_object(&pending, final_digest).expect("bind");
        assert_eq!(revision.revision_id, pending.revision_id);
        assert_eq!(revision.occurrence_sequence, pending.occurrence_sequence);
        assert_eq!(revision.content_digest, pending.content_digest);
        assert_eq!(revision.byte_length, pending.byte_length);
        assert_eq!(revision.observed_at, pending.observed_at);
        assert_eq!(revision.acquisition_kind, pending.acquisition_kind);
        assert_eq!(revision.stability_receipt_ref, pending.stability_receipt_ref);
        assert_eq!(revision.object_residency_key_digest, final_digest);
    }

    #[test]
    fn bind_pending_to_object_rejects_non_v4_uuid() {
        let mut pending = pending_descriptor("op:test");
        pending.revision_id = SourceRevisionId::from_bytes([0x00; 16]);
        let final_digest = ObjectResidencyKeyDigest::from_bytes([0x99; 32]);
        assert!(bind_pending_to_object(&pending, final_digest).is_err());
    }

    #[test]
    fn pending_survives_late_binding() {
        let mut port = TestPreparePort::default();
        let descriptor = pending_descriptor("op:test");
        port.store_pending_revision(&descriptor, &context()).expect("store");
        let final_digest = ObjectResidencyKeyDigest::from_bytes([0x99; 32]);
        let _revision = bind_pending_to_object(&descriptor, final_digest).expect("bind");
        let loaded = port
            .load_pending_revision(&descriptor.operation_id, &context())
            .expect("load")
            .expect("pending survives");
        assert_eq!(loaded, descriptor);
    }

    #[test]
    fn pending_replay_does_not_redraw_uuid() {
        let mut port = TestPreparePort::default();
        let descriptor = pending_descriptor("op:test");
        port.store_pending_revision(&descriptor, &context()).expect("store");
        let loaded = port
            .load_pending_revision(&descriptor.operation_id, &context())
            .expect("load")
            .expect("pending exists");
        assert_eq!(loaded.revision_id, descriptor.revision_id);
        assert_eq!(loaded.occurrence_sequence, descriptor.occurrence_sequence);
        assert_eq!(loaded.observed_at, descriptor.observed_at);
    }

    #[test]
    fn pending_closure_mismatch_is_detected() {
        let mut port = TestPreparePort::default();
        let descriptor = pending_descriptor("op:test");
        port.store_pending_revision(&descriptor, &context()).expect("store");
        let mut different_request = preseal_request("op:test");
        different_request.residency_closure = vec![0x55, 0x56];
        assert!(!pending_matches_request(&descriptor, &different_request));
    }
}

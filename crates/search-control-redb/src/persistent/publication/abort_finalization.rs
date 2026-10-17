//! Bounded C02 metadata retained with abort finalization in the existing journal.
//!
//! This record carries only operation/context/guard values and references or
//! digests for source-backed publication data. It never stores manifests,
//! manifest diffs, point sets, payloads, or vectors.

use super::{
    Blake3Digest32, Check, ControlError, ControlKey, ControlRecordClass, ControlValue,
    JournalLimits, Point,
};
use super::abandonment::PublicationAbandonFenceRecordV1;
use crate::persistent::bootstrap::OriginalCheck;
use crate::{MigrationBootstrapFailure, MigrationBootstrapWork as Work, MutationId};
use search_contracts::{
    CollectionGenerationId, Epoch, MAX_OPAQUE_ID_BYTES, MAX_OPAQUE_REF_BYTES, OpaqueId, OpaqueRef,
    OwnerEpoch, ProjectionMembershipId, PublicationGuards, PublicationIntent, PublicationIntentId,
    ReceiptRef, RequestId,
};
use std::time::Instant;

const KEY: &[u8] = b"publication_intents/abort-finalization/v1";
const MAGIC: &[u8; 8] = b"ELIPAB01";
const MAGIC_WITH_FENCE: &[u8; 8] = b"ELIPAB02";
const MAX_RECORD_BYTES: usize = 64 * 1024;

/// Compact outcome metadata. Excluded memberships are rehydrated from the
/// admitted manifest diff and owner readback; only their count and digest live
/// in the control journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AbortResolutionMetadataV1 {
    /// The exact compensation operation restored the prior bounds.
    Compensated {
        /// Whether the original diff contained old points to restore.
        restored_old_points: bool,
    },
    /// The exact membership fence was read back from its owning control state.
    Excluded {
        /// Number of exact memberships in the retained, source-backed diff.
        membership_count: u64,
        /// Digest of those canonical membership IDs.
        membership_digest: Blake3Digest32,
    },
}

/// Bounded abort request and binding metadata for the existing C02 journal.
///
/// The full publication binding is reconstructed from the re-admitted C07/C13
/// originals. This record deliberately contains only the C13 reference, compact
/// identities, finite context, guards, and digests.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationAbortFinalizationMetadataV1 {
    /// Stable abort request operation identity.
    pub abort_operation_id: OpaqueId,
    /// C02 mutation identity used to commit the abort finalization.
    pub c02_mutation_id: MutationId,
    /// C02 mutation command digest, including the original abort request.
    pub c02_command_digest: Blake3Digest32,
    /// Generation expected by the abort finalization transaction.
    pub expected_control_generation: u64,
    /// Digest of every field in the original abort request, including any
    /// membership list represented by the bounded resolution digest below.
    pub abort_request_digest: Blake3Digest32,
    /// Original C02 publication intent identity.
    pub publication_intent_id: PublicationIntentId,
    /// Original immutable publication transaction identity.
    pub transaction_id: OpaqueId,
    /// Exact collection route.
    pub collection_generation_id: CollectionGenerationId,
    /// Membership whose exact old/new references are retained below.
    pub projection_membership_id: ProjectionMembershipId,
    /// Full physical schema identity digest.
    pub schema_identity_digest: Blake3Digest32,
    /// Original reserved target epoch.
    pub target_epoch: Epoch,
    /// Exact visible predecessor retained by the original publication.
    pub previous_visible_epoch: Epoch,
    /// Original old manifest identity, if any.
    pub old_manifest_digest: Option<Blake3Digest32>,
    /// Original new manifest identity.
    pub new_manifest_digest: Blake3Digest32,
    /// Existing C02 reference for the old source-backed C13 set, if any.
    pub old_manifest_reference: Option<ReceiptRef>,
    /// Existing C02 reference for the original source-backed C13 set.
    pub new_manifest_reference: ReceiptRef,
    /// Digest of the complete admitted scope.
    pub scope_digest: Blake3Digest32,
    /// Complete prepared-generation identity, including derived diff inputs.
    pub prepared_input_digest: Blake3Digest32,
    /// Original intent operation identity.
    pub original_persist_operation_id: OpaqueId,
    /// Request identity from the original operation context.
    pub original_request_id: RequestId,
    /// Original finite relative deadline from the intent context.
    pub original_relative_deadline_ms: u64,
    /// Original opaque budget reference from the intent context.
    pub original_budget_ref: OpaqueRef,
    /// Request identity for this exact abort-finalization attempt.
    pub abort_request_id: RequestId,
    /// Finite relative deadline for this exact abort-finalization attempt.
    pub abort_relative_deadline_ms: u64,
    /// Opaque budget reference for this exact abort-finalization attempt.
    pub abort_budget_ref: OpaqueRef,
    /// Durable C02 generation that recorded the original intent.
    pub original_control_generation: u64,
    /// Digest of the original durable C02 intent state.
    pub original_control_state_digest: Blake3Digest32,
    /// Guards admitted with the original intent.
    pub original_guards: PublicationGuards,
    /// Guards revalidated by this abort request.
    pub current_guards: PublicationGuards,
    /// Stable operation identity of the exact compensation/exclusion readback.
    pub resolution_operation_id: OpaqueId,
    /// Compact outcome metadata for reconstructing the exact original request.
    pub resolution: AbortResolutionMetadataV1,
}

impl core::fmt::Debug for PublicationAbortFinalizationMetadataV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PublicationAbortFinalizationMetadataV1([REDACTED])")
    }
}

/// Strictly encoded immutable C02 abort metadata record.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationAbortFinalizationRecordV1 {
    metadata: PublicationAbortFinalizationMetadataV1,
    abandonment_fence: Option<Box<PublicationAbandonFenceRecordV1>>,
    canonical: Vec<u8>,
}

impl core::fmt::Debug for PublicationAbortFinalizationRecordV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PublicationAbortFinalizationRecordV1([REDACTED])")
    }
}

impl PublicationAbortFinalizationRecordV1 {
    /// Validates and encodes compact metadata under the original operation budget.
    ///
    /// # Errors
    /// Rejects invalid generation/epoch/context values, oversized metadata, or
    /// any refusal from the original operation context.
    pub fn prepare_with_original_context<F, E>(
        metadata: PublicationAbortFinalizationMetadataV1,
        limits: JournalLimits,
        deadline: Instant,
        work: &F,
    ) -> Result<Self, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            let record = Self::prepare(metadata, None, limits, &check)?;
            check.check(Point::ReadComplete)?;
            Ok(record)
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Prepares an excluded finalization only when accompanied by the exact
    /// retained C02 fence record returned by the owner readback.
    ///
    /// # Errors
    /// Rejects mismatched operation, request, intent, route, guard or membership
    /// metadata, oversized values, and any refusal from the original allowance.
    pub fn prepare_with_abandonment_fence_v1<F, E>(
        metadata: PublicationAbortFinalizationMetadataV1,
        fence: PublicationAbandonFenceRecordV1,
        limits: JournalLimits,
        deadline: Instant,
        work: &F,
    ) -> Result<Self, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            let record = Self::prepare(metadata, Some(fence), limits, &check)?;
            check.check(Point::ReadComplete)?;
            Ok(record)
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Exact retained compact metadata; it is not a publication proof.
    #[must_use]
    pub const fn metadata(&self) -> &PublicationAbortFinalizationMetadataV1 {
        &self.metadata
    }

    /// Exact C02 membership fence retained with an excluded abort, if any.
    #[must_use]
    pub fn abandonment_fence(&self) -> Option<&PublicationAbandonFenceRecordV1> {
        self.abandonment_fence.as_deref()
    }

    pub(in crate::persistent) fn value(
        &self,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<ControlValue, ControlError> {
        check.check(Point::Start)?;
        check.reserve(Work::Allocate(self.canonical.len()))?;
        check.reserve(Work::Process(self.canonical.len()))?;
        ControlValue::new(
            ControlRecordClass::Operation,
            self.canonical.clone(),
            limits,
        )
    }

    pub(in crate::persistent) fn decode(
        value: &ControlValue,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        check.check(Point::ReadRecord)?;
        let bytes = value.as_bytes();
        if value.class() != ControlRecordClass::Operation
            || bytes.len() < MAGIC.len()
            || bytes.len() > MAX_RECORD_BYTES
            || bytes.len() > limits.max_value_bytes
        {
            return Err(ControlError::StoreCorrupt);
        }
        check.reserve(Work::Allocate(
            bytes
                .len()
                .checked_mul(2)
                .ok_or(ControlError::BudgetExceeded)?,
        ))?;
        check.reserve(Work::Process(bytes.len()))?;
        let mut input = Input(bytes);
        let magic = input.take::<8>()?;
        let has_fence = if &magic == MAGIC {
            false
        } else if &magic == MAGIC_WITH_FENCE {
            true
        } else {
            return Err(ControlError::StoreCorrupt);
        };
        let metadata = decode_metadata(&mut input)?;
        let abandonment_fence = if has_fence {
            let length = usize::try_from(input.u32()?).map_err(|_| ControlError::StoreCorrupt)?;
            let nested = input.take_vec(length)?;
            let nested_value = ControlValue::new(ControlRecordClass::Operation, nested, limits)?;
            Some(Box::new(PublicationAbandonFenceRecordV1::decode(
                &nested_value,
                limits,
                check,
            )?))
        } else {
            None
        };
        if !input.is_empty() {
            return Err(ControlError::StoreCorrupt);
        }
        validate_metadata(&metadata).map_err(|_| ControlError::StoreCorrupt)?;
        validate_fence_binding(&metadata, abandonment_fence.as_deref())
            .map_err(|_| ControlError::StoreCorrupt)?;
        let canonical = bytes.to_vec();
        let record = Self {
            metadata,
            abandonment_fence,
            canonical,
        };
        check.check(Point::ReadComplete)?;
        Ok(record)
    }

    pub(in crate::persistent) fn matches_intent(&self, intent: &PublicationIntent) -> bool {
        let metadata = &self.metadata;
        intent.state == search_contracts::PublicationIntentState::Aborted
            && metadata.publication_intent_id == intent.publication_intent_id
            && metadata.target_epoch == intent.target_epoch
            && metadata.original_guards == intent.owner_source_membership_access_guards
            && metadata.new_manifest_reference == intent.prepared_manifest_ref
    }

    fn prepare(
        metadata: PublicationAbortFinalizationMetadataV1,
        abandonment_fence: Option<PublicationAbandonFenceRecordV1>,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        validate_metadata(&metadata)?;
        validate_fence_binding(&metadata, abandonment_fence.as_ref())?;
        check.reserve(Work::Allocate(MAX_RECORD_BYTES))?;
        let mut canonical = Vec::with_capacity(MAX_RECORD_BYTES);
        canonical.extend_from_slice(if abandonment_fence.is_some() {
            MAGIC_WITH_FENCE
        } else {
            MAGIC
        });
        encode_metadata(&metadata, &mut canonical)?;
        if let Some(fence) = &abandonment_fence {
            let nested = fence.value(limits, check)?;
            let length = u32::try_from(nested.len()).map_err(|_| ControlError::BudgetExceeded)?;
            canonical.extend_from_slice(&length.to_be_bytes());
            canonical.extend_from_slice(nested.as_bytes());
        }
        if canonical.len() > MAX_RECORD_BYTES || canonical.len() > limits.max_value_bytes {
            return Err(ControlError::InvalidValue);
        }
        check.reserve(Work::Process(canonical.len()))?;
        Ok(Self {
            metadata,
            abandonment_fence: abandonment_fence.map(Box::new),
            canonical,
        })
    }
}

pub(in crate::persistent) fn key(limits: JournalLimits) -> Result<ControlKey, ControlError> {
    ControlKey::new(KEY.to_vec(), limits)
}

pub(in crate::persistent) fn reserved(key: &[u8]) -> bool {
    key == KEY
}

pub(in crate::persistent) fn read_snapshot(
    snapshot: &crate::JournalReadSnapshot,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<Option<PublicationAbortFinalizationRecordV1>, ControlError> {
    let Some(value) = snapshot.get(&key(limits)?) else {
        return Ok(None);
    };
    if super::abandonment::is_fence_value(value) {
        return Ok(None);
    }
    PublicationAbortFinalizationRecordV1::decode(value, limits, check).map(Some)
}

fn validate_fence_binding(
    metadata: &PublicationAbortFinalizationMetadataV1,
    fence: Option<&PublicationAbandonFenceRecordV1>,
) -> Result<(), ControlError> {
    match (&metadata.resolution, fence) {
        (AbortResolutionMetadataV1::Compensated { .. }, None) => Ok(()),
        (AbortResolutionMetadataV1::Excluded { .. }, Some(fence))
            if fence.matches_abort(metadata) => Ok(()),
        _ => Err(ControlError::OperationConflict),
    }
}
fn validate_metadata(
    metadata: &PublicationAbortFinalizationMetadataV1,
) -> Result<(), ControlError> {
    let generation = metadata.expected_control_generation;
    if generation == 0
        || generation.checked_add(1).is_none()
        || metadata.original_control_generation == 0
        || metadata.original_control_generation > generation
        || metadata.original_relative_deadline_ms == 0
        || metadata.abort_relative_deadline_ms == 0
        || metadata.target_epoch.get() <= 0
        || metadata.previous_visible_epoch.get() < 0
        || metadata.abort_operation_id == metadata.original_persist_operation_id
        || metadata.abort_operation_id == metadata.resolution_operation_id
        || metadata.original_guards.profile_digest != metadata.current_guards.profile_digest
        || metadata.current_guards.owner_epoch < metadata.original_guards.owner_epoch
        || metadata.current_guards.source_catalog_generation
            < metadata.original_guards.source_catalog_generation
        || metadata.current_guards.membership_generation
            < metadata.original_guards.membership_generation
        || metadata.current_guards.access_generation < metadata.original_guards.access_generation
        || metadata.current_guards.shadow_generation < metadata.original_guards.shadow_generation
        || metadata.current_guards.purge_generation < metadata.original_guards.purge_generation
        || metadata.old_manifest_digest.is_some() != metadata.old_manifest_reference.is_some()
    {
        return Err(ControlError::InvalidValue);
    }
    match &metadata.resolution {
        AbortResolutionMetadataV1::Compensated { .. } => {}
        AbortResolutionMetadataV1::Excluded {
            membership_count, ..
        } if *membership_count > 0 => {}
        AbortResolutionMetadataV1::Excluded { .. } => return Err(ControlError::InvalidValue),
    }
    Ok(())
}

fn encode_metadata(
    metadata: &PublicationAbortFinalizationMetadataV1,
    out: &mut Vec<u8>,
) -> Result<(), ControlError> {
    put_text(
        out,
        metadata.abort_operation_id.as_str(),
        MAX_OPAQUE_ID_BYTES,
    )?;
    out.extend_from_slice(&metadata.c02_mutation_id.0);
    out.extend_from_slice(metadata.c02_command_digest.as_bytes());
    out.extend_from_slice(&metadata.expected_control_generation.to_be_bytes());
    out.extend_from_slice(metadata.abort_request_digest.as_bytes());
    out.extend_from_slice(metadata.publication_intent_id.as_bytes());
    put_text(out, metadata.transaction_id.as_str(), MAX_OPAQUE_ID_BYTES)?;
    out.extend_from_slice(metadata.collection_generation_id.as_bytes());
    out.extend_from_slice(metadata.projection_membership_id.as_bytes());
    out.extend_from_slice(metadata.schema_identity_digest.as_bytes());
    out.extend_from_slice(&metadata.target_epoch.get().to_be_bytes());
    out.extend_from_slice(&metadata.previous_visible_epoch.get().to_be_bytes());
    put_optional_digest(out, metadata.old_manifest_digest);
    out.extend_from_slice(metadata.new_manifest_digest.as_bytes());
    put_optional_text(
        out,
        metadata
            .old_manifest_reference
            .as_ref()
            .map(ReceiptRef::as_str),
        MAX_OPAQUE_REF_BYTES,
    )?;
    put_text(
        out,
        metadata.new_manifest_reference.as_str(),
        MAX_OPAQUE_REF_BYTES,
    )?;
    out.extend_from_slice(metadata.scope_digest.as_bytes());
    out.extend_from_slice(metadata.prepared_input_digest.as_bytes());
    put_text(
        out,
        metadata.original_persist_operation_id.as_str(),
        MAX_OPAQUE_ID_BYTES,
    )?;
    out.extend_from_slice(metadata.original_request_id.as_bytes());
    out.extend_from_slice(&metadata.original_relative_deadline_ms.to_be_bytes());
    put_text(
        out,
        metadata.original_budget_ref.as_str(),
        MAX_OPAQUE_REF_BYTES,
    )?;
    out.extend_from_slice(metadata.abort_request_id.as_bytes());
    out.extend_from_slice(&metadata.abort_relative_deadline_ms.to_be_bytes());
    put_text(
        out,
        metadata.abort_budget_ref.as_str(),
        MAX_OPAQUE_REF_BYTES,
    )?;
    out.extend_from_slice(&metadata.original_control_generation.to_be_bytes());
    out.extend_from_slice(metadata.original_control_state_digest.as_bytes());
    encode_guards(out, metadata.original_guards);
    encode_guards(out, metadata.current_guards);
    put_text(
        out,
        metadata.resolution_operation_id.as_str(),
        MAX_OPAQUE_ID_BYTES,
    )?;
    match &metadata.resolution {
        AbortResolutionMetadataV1::Compensated {
            restored_old_points,
        } => {
            out.push(1);
            out.push(u8::from(*restored_old_points));
        }
        AbortResolutionMetadataV1::Excluded {
            membership_count,
            membership_digest,
        } => {
            out.push(2);
            out.extend_from_slice(&membership_count.to_be_bytes());
            out.extend_from_slice(membership_digest.as_bytes());
        }
    }
    Ok(())
}

fn decode_metadata(
    input: &mut Input<'_>,
) -> Result<PublicationAbortFinalizationMetadataV1, ControlError> {
    let abort_operation_id =
        OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?;
    let c02_mutation_id = MutationId(input.take()?);
    let c02_command_digest = Blake3Digest32::from_bytes(input.take()?);
    let expected_control_generation = input.u64()?;
    let abort_request_digest = Blake3Digest32::from_bytes(input.take()?);
    let publication_intent_id = PublicationIntentId::from_bytes(input.take()?);
    let transaction_id =
        OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?;
    let collection_generation_id = CollectionGenerationId::from_bytes(input.take()?);
    let projection_membership_id = ProjectionMembershipId::from_bytes(input.take()?);
    let schema_identity_digest = Blake3Digest32::from_bytes(input.take()?);
    let target_epoch = Epoch::new(input.i64()?).map_err(|_| ControlError::StoreCorrupt)?;
    let previous_visible_epoch =
        Epoch::new(input.i64()?).map_err(|_| ControlError::StoreCorrupt)?;
    let old_manifest_digest = input.optional_digest()?;
    let new_manifest_digest = Blake3Digest32::from_bytes(input.take()?);
    let old_manifest_reference = input
        .optional_text(MAX_OPAQUE_REF_BYTES)?
        .map(ReceiptRef::new)
        .transpose()
        .map_err(|_| ControlError::StoreCorrupt)?;
    let new_manifest_reference = ReceiptRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let scope_digest = Blake3Digest32::from_bytes(input.take()?);
    let prepared_input_digest = Blake3Digest32::from_bytes(input.take()?);
    let original_persist_operation_id =
        OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?;
    let original_request_id = RequestId::from_bytes(input.take()?);
    let original_relative_deadline_ms = input.u64()?;
    let original_budget_ref = OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let abort_request_id = RequestId::from_bytes(input.take()?);
    let abort_relative_deadline_ms = input.u64()?;
    let abort_budget_ref = OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let original_control_generation = input.u64()?;
    let original_control_state_digest = Blake3Digest32::from_bytes(input.take()?);
    let original_guards = decode_guards(input)?;
    let current_guards = decode_guards(input)?;
    let resolution_operation_id =
        OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?;
    let resolution = match input.u8()? {
        1 => match input.u8()? {
            0 => AbortResolutionMetadataV1::Compensated {
                restored_old_points: false,
            },
            1 => AbortResolutionMetadataV1::Compensated {
                restored_old_points: true,
            },
            _ => return Err(ControlError::StoreCorrupt),
        },
        2 => AbortResolutionMetadataV1::Excluded {
            membership_count: input.u64()?,
            membership_digest: Blake3Digest32::from_bytes(input.take()?),
        },
        _ => return Err(ControlError::StoreCorrupt),
    };
    Ok(PublicationAbortFinalizationMetadataV1 {
        abort_operation_id,
        c02_mutation_id,
        c02_command_digest,
        expected_control_generation,
        abort_request_digest,
        publication_intent_id,
        transaction_id,
        collection_generation_id,
        projection_membership_id,
        schema_identity_digest,
        target_epoch,
        previous_visible_epoch,
        old_manifest_digest,
        new_manifest_digest,
        old_manifest_reference,
        new_manifest_reference,
        scope_digest,
        prepared_input_digest,
        original_persist_operation_id,
        original_request_id,
        original_relative_deadline_ms,
        original_budget_ref,
        abort_request_id,
        abort_relative_deadline_ms,
        abort_budget_ref,
        original_control_generation,
        original_control_state_digest,
        original_guards,
        current_guards,
        resolution_operation_id,
        resolution,
    })
}

fn encode_guards(out: &mut Vec<u8>, guards: PublicationGuards) {
    for value in [
        guards.owner_epoch.get(),
        guards.source_catalog_generation,
        guards.membership_generation,
        guards.access_generation,
        guards.shadow_generation,
        guards.purge_generation,
    ] {
        out.extend_from_slice(&value.to_be_bytes());
    }
    out.extend_from_slice(guards.profile_digest.as_bytes());
}

fn decode_guards(input: &mut Input<'_>) -> Result<PublicationGuards, ControlError> {
    Ok(PublicationGuards {
        owner_epoch: OwnerEpoch::new(input.u64()?).map_err(|_| ControlError::StoreCorrupt)?,
        source_catalog_generation: input.u64()?,
        membership_generation: input.u64()?,
        access_generation: input.u64()?,
        shadow_generation: input.u64()?,
        purge_generation: input.u64()?,
        profile_digest: Blake3Digest32::from_bytes(input.take()?),
    })
}

fn put_optional_digest(out: &mut Vec<u8>, digest: Option<Blake3Digest32>) {
    match digest {
        Some(digest) => {
            out.push(1);
            out.extend_from_slice(digest.as_bytes());
        }
        None => out.push(0),
    }
}

fn put_optional_text(
    out: &mut Vec<u8>,
    value: Option<&str>,
    max: usize,
) -> Result<(), ControlError> {
    if let Some(value) = value {
        out.push(1);
        put_text(out, value, max)
    } else {
        out.push(0);
        Ok(())
    }
}
fn put_text(out: &mut Vec<u8>, value: &str, max: usize) -> Result<(), ControlError> {
    if value.is_empty() || value.len() > max {
        return Err(ControlError::InvalidValue);
    }
    let length = u16::try_from(value.len()).map_err(|_| ControlError::InvalidValue)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Input<'a>(&'a [u8]);

impl Input<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        let (head, tail) = self
            .0
            .split_at_checked(N)
            .ok_or(ControlError::StoreCorrupt)?;
        self.0 = tail;
        head.try_into().map_err(|_| ControlError::StoreCorrupt)
    }

    fn u8(&mut self) -> Result<u8, ControlError> {
        Ok(self.take::<1>()?[0])
    }

    fn u64(&mut self) -> Result<u64, ControlError> {
        Ok(u64::from_be_bytes(self.take()?))
    }

    fn i64(&mut self) -> Result<i64, ControlError> {
        Ok(i64::from_be_bytes(self.take()?))
    }

    fn u32(&mut self) -> Result<u32, ControlError> {
        Ok(u32::from_be_bytes(self.take()?))
    }

    fn text(&mut self, max: usize) -> Result<String, ControlError> {
        let length = usize::from(u16::from_be_bytes(self.take()?));
        if length == 0 || length > max {
            return Err(ControlError::StoreCorrupt);
        }
        String::from_utf8(self.take_bytes(length)?.to_vec()).map_err(|_| ControlError::StoreCorrupt)
    }

    fn optional_text(&mut self, max: usize) -> Result<Option<String>, ControlError> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.text(max).map(Some),
            _ => Err(ControlError::StoreCorrupt),
        }
    }

    fn optional_digest(&mut self) -> Result<Option<Blake3Digest32>, ControlError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(Blake3Digest32::from_bytes(self.take()?))),
            _ => Err(ControlError::StoreCorrupt),
        }
    }

    fn take_bytes(&mut self, length: usize) -> Result<&[u8], ControlError> {
        let (head, tail) = self
            .0
            .split_at_checked(length)
            .ok_or(ControlError::StoreCorrupt)?;
        self.0 = tail;
        Ok(head)
    }

    fn take_vec(&mut self, length: usize) -> Result<Vec<u8>, ControlError> {
        if length == 0 || length > MAX_RECORD_BYTES {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(self.take_bytes(length)?.to_vec())
    }

    const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

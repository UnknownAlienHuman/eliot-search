//! Finite original-operation metadata retained with the current publication intent.
//!
//! This record preserves only the original mutation/context coordinates and the
//! C02 intent coordinates needed to rehydrate a vendor-neutral binding from the
//! retained C07/C13 sources. It contains no manifests, diffs, points, payloads,
//! or vectors.

use super::{
    Blake3Digest32, Check, ControlError, ControlKey, ControlRecordClass, ControlValue,
    JournalLimits, Point,
};
use crate::persistent::bootstrap::OriginalCheck;
use crate::{MigrationBootstrapFailure, MigrationBootstrapWork as Work, MutationId};
use search_contracts::{
    Epoch, MAX_OPAQUE_ID_BYTES, MAX_OPAQUE_REF_BYTES, OpaqueId, OpaqueRef, PublicationGuards,
    PublicationIntent, PublicationIntentId, PublicationReceiptId, ReceiptRef, RequestId,
};
use std::time::Instant;

const KEY: &[u8] = b"publication_intents/original-binding/v1";
const MAGIC: &[u8; 8] = b"ELIPOB01";
const MAX_RECORD_BYTES: usize = 1024;

/// Bounded coordinates for rehydrating the original intent binding.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationIntentBindingMetadataV1 {
    /// C02 intent identity this original binding belongs to.
    pub publication_intent_id: PublicationIntentId,
    /// Root-issued receipt identity reserved for the original visibility CAS.
    /// It is retained before effects so a lost reply cannot redraw it.
    pub publication_receipt_id: PublicationReceiptId,
    /// Exact reserved epoch from the C02 intent.
    pub target_epoch: Epoch,
    /// Existing C13/C02 manifest reference from the C02 intent.
    pub prepared_manifest_reference: ReceiptRef,
    /// Original intent guards from the C02 intent.
    pub guards: PublicationGuards,
    /// C02 mutation identity that durably created this intent.
    pub c02_mutation_id: MutationId,
    /// Exact command digest for that C02 mutation.
    pub c02_command_digest: Blake3Digest32,
    /// Expected C02 generation before the original intent mutation.
    pub expected_control_generation: u64,
    /// Original model mutation identity, distinct from the C02 mutation ID.
    pub original_operation_id: OpaqueId,
    /// Original request identity from the publication `OperationContext`.
    pub original_request_id: RequestId,
    /// Original finite relative deadline from that context.
    pub original_relative_deadline_ms: u64,
    /// Original opaque budget reference from that context.
    pub original_budget_ref: OpaqueRef,
}

impl core::fmt::Debug for PublicationIntentBindingMetadataV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PublicationIntentBindingMetadataV1([REDACTED])")
    }
}

/// Strictly encoded finite original-binding record in the existing C02 journal.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationIntentBindingRecordV1 {
    metadata: PublicationIntentBindingMetadataV1,
    canonical: Vec<u8>,
}

impl core::fmt::Debug for PublicationIntentBindingRecordV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PublicationIntentBindingRecordV1([REDACTED])")
    }
}

impl PublicationIntentBindingRecordV1 {
    /// Validates and encodes finite original binding coordinates under the original context.
    ///
    /// # Errors
    /// Refuses invalid generations/deadlines, oversized records, or original-work refusal.
    pub fn prepare_with_original_context<F, E>(
        metadata: PublicationIntentBindingMetadataV1,
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
            let record = Self::prepare(metadata, limits, &check)?;
            check.check(Point::ReadComplete)?;
            Ok(record)
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Exact finite metadata; it is not publication proof.
    #[must_use]
    pub const fn metadata(&self) -> &PublicationIntentBindingMetadataV1 {
        &self.metadata
    }

    /// Whether this record names the exact complete C02 intent fields.
    #[must_use]
    pub fn matches_intent(&self, intent: &PublicationIntent) -> bool {
        let metadata = &self.metadata;
        if metadata.publication_intent_id != intent.publication_intent_id {
            return false;
        }
        if metadata.target_epoch != intent.target_epoch {
            return false;
        }
        if metadata.prepared_manifest_reference != intent.prepared_manifest_ref {
            return false;
        }
        metadata.guards == intent.owner_source_membership_access_guards
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
        if &input.take::<8>()? != MAGIC {
            return Err(ControlError::StoreCorrupt);
        }
        let metadata = decode_metadata(&mut input)?;
        if !input.is_empty() {
            return Err(ControlError::StoreCorrupt);
        }
        let record = Self::prepare(metadata, limits, check)?;
        if record.canonical != bytes {
            return Err(ControlError::StoreCorrupt);
        }
        check.check(Point::ReadComplete)?;
        Ok(record)
    }

    fn prepare(
        metadata: PublicationIntentBindingMetadataV1,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        if metadata.expected_control_generation == 0
            || metadata
                .expected_control_generation
                .checked_add(1)
                .is_none()
            || metadata.original_relative_deadline_ms == 0
            || metadata.publication_intent_id.as_bytes()
                == metadata.publication_receipt_id.as_bytes()
        {
            return Err(ControlError::InvalidValue);
        }
        check.reserve(Work::Allocate(MAX_RECORD_BYTES))?;
        let mut canonical = Vec::with_capacity(MAX_RECORD_BYTES);
        canonical.extend_from_slice(MAGIC);
        encode_metadata(&metadata, &mut canonical)?;
        if canonical.len() > MAX_RECORD_BYTES || canonical.len() > limits.max_value_bytes {
            return Err(ControlError::InvalidValue);
        }
        check.reserve(Work::Process(canonical.len()))?;
        Ok(Self {
            metadata,
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
) -> Result<Option<PublicationIntentBindingRecordV1>, ControlError> {
    snapshot
        .get(&key(limits)?)
        .map(|value| PublicationIntentBindingRecordV1::decode(value, limits, check))
        .transpose()
}

fn encode_metadata(
    metadata: &PublicationIntentBindingMetadataV1,
    out: &mut Vec<u8>,
) -> Result<(), ControlError> {
    out.extend_from_slice(metadata.publication_intent_id.as_bytes());
    out.extend_from_slice(metadata.publication_receipt_id.as_bytes());
    out.extend_from_slice(&metadata.target_epoch.get().to_be_bytes());
    put_text(
        out,
        metadata.prepared_manifest_reference.as_str(),
        MAX_OPAQUE_REF_BYTES,
    )?;
    encode_guards(out, metadata.guards);
    out.extend_from_slice(&metadata.c02_mutation_id.0);
    out.extend_from_slice(metadata.c02_command_digest.as_bytes());
    out.extend_from_slice(&metadata.expected_control_generation.to_be_bytes());
    put_text(
        out,
        metadata.original_operation_id.as_str(),
        MAX_OPAQUE_ID_BYTES,
    )?;
    out.extend_from_slice(metadata.original_request_id.as_bytes());
    out.extend_from_slice(&metadata.original_relative_deadline_ms.to_be_bytes());
    put_text(
        out,
        metadata.original_budget_ref.as_str(),
        MAX_OPAQUE_REF_BYTES,
    )
}

fn decode_metadata(
    input: &mut Input<'_>,
) -> Result<PublicationIntentBindingMetadataV1, ControlError> {
    let publication_intent_id = PublicationIntentId::from_bytes(input.take()?);
    let publication_receipt_id = PublicationReceiptId::from_bytes(input.take()?);
    let target_epoch = Epoch::new(input.i64()?).map_err(|_| ControlError::StoreCorrupt)?;
    let prepared_manifest_reference = ReceiptRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let guards = decode_guards(input)?;
    let c02_mutation_id = MutationId(input.take()?);
    let c02_command_digest = Blake3Digest32::from_bytes(input.take()?);
    let expected_control_generation = input.u64()?;
    let original_operation_id =
        OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?;
    let original_request_id = RequestId::from_bytes(input.take()?);
    let original_relative_deadline_ms = input.u64()?;
    let original_budget_ref = OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    Ok(PublicationIntentBindingMetadataV1 {
        publication_intent_id,
        publication_receipt_id,
        target_epoch,
        prepared_manifest_reference,
        guards,
        c02_mutation_id,
        c02_command_digest,
        expected_control_generation,
        original_operation_id,
        original_request_id,
        original_relative_deadline_ms,
        original_budget_ref,
    })
}

fn encode_guards(out: &mut Vec<u8>, guards: PublicationGuards) {
    out.extend_from_slice(&guards.owner_epoch.get().to_be_bytes());
    for generation in [
        guards.source_catalog_generation,
        guards.membership_generation,
        guards.access_generation,
        guards.shadow_generation,
        guards.purge_generation,
    ] {
        out.extend_from_slice(&generation.to_be_bytes());
    }
    out.extend_from_slice(guards.profile_digest.as_bytes());
}

fn decode_guards(input: &mut Input<'_>) -> Result<PublicationGuards, ControlError> {
    Ok(PublicationGuards {
        owner_epoch: search_contracts::OwnerEpoch::new(input.u64()?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        source_catalog_generation: input.u64()?,
        membership_generation: input.u64()?,
        access_generation: input.u64()?,
        shadow_generation: input.u64()?,
        purge_generation: input.u64()?,
        profile_digest: Blake3Digest32::from_bytes(input.take()?),
    })
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

    fn u64(&mut self) -> Result<u64, ControlError> {
        Ok(u64::from_be_bytes(self.take()?))
    }

    fn i64(&mut self) -> Result<i64, ControlError> {
        Ok(i64::from_be_bytes(self.take()?))
    }

    fn text(&mut self, max: usize) -> Result<String, ControlError> {
        let length = usize::from(u16::from_be_bytes(self.take()?));
        if length == 0 || length > max {
            return Err(ControlError::StoreCorrupt);
        }
        let (head, tail) = self
            .0
            .split_at_checked(length)
            .ok_or(ControlError::StoreCorrupt)?;
        self.0 = tail;
        String::from_utf8(head.to_vec()).map_err(|_| ControlError::StoreCorrupt)
    }

    const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

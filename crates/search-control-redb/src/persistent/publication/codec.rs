//! Closed, versioned technical intent value; no JSON or defaulted guard fields.

use search_contracts::{Blake3Digest32, Epoch, MAX_OPAQUE_REF_BYTES, OwnerEpoch,
    PublicationGuards, PublicationIntent, PublicationIntentId, PublicationIntentState, ReceiptRef};
use super::{ControlError, ControlRecordClass, ControlValue, JournalLimits};

const MAGIC: &[u8; 8] = b"ELIPUB01";
const FIXED_BYTES: usize = 8 + 16 + 8 + 1 + 6 * 8 + 32 + 2;
const MAX_BYTES: usize = FIXED_BYTES + MAX_OPAQUE_REF_BYTES;

pub(super) fn encode(intent: &PublicationIntent, limits: JournalLimits) -> Result<ControlValue, ControlError> {
    let reference = intent.prepared_manifest_ref.as_str().as_bytes();
    let length = u16::try_from(reference.len()).map_err(|_| ControlError::InvalidValue)?;
    let total = FIXED_BYTES.checked_add(reference.len()).ok_or(ControlError::BudgetExceeded)?;
    if intent.target_epoch.get() == 0 || reference.is_empty() || total > MAX_BYTES || total > limits.max_value_bytes {
        return Err(ControlError::InvalidValue);
    }
    let guards = intent.owner_source_membership_access_guards;
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(intent.publication_intent_id.as_bytes());
    bytes.extend_from_slice(&intent.target_epoch.get().to_be_bytes());
    bytes.push(state_tag(intent.state));
    for counter in [guards.owner_epoch.get(), guards.source_catalog_generation,
        guards.membership_generation, guards.access_generation, guards.shadow_generation,
        guards.purge_generation] { bytes.extend_from_slice(&counter.to_be_bytes()); }
    bytes.extend_from_slice(guards.profile_digest.as_bytes());
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(reference);
    ControlValue::new(ControlRecordClass::Operation, bytes, limits)
}

pub(super) fn decode(value: &ControlValue) -> Result<PublicationIntent, ControlError> {
    let bytes = value.as_bytes();
    if value.class() != ControlRecordClass::Operation || bytes.len() < FIXED_BYTES || bytes.len() > MAX_BYTES {
        return Err(ControlError::StoreCorrupt);
    }
    let mut input = Input(bytes);
    if &input.take::<8>()? != MAGIC { return Err(ControlError::StoreCorrupt); }
    let id = PublicationIntentId::from_bytes(input.take()?);
    let epoch = Epoch::new(i64::from_be_bytes(input.take()?)).map_err(|_| ControlError::StoreCorrupt)?;
    if epoch.get() == 0 { return Err(ControlError::StoreCorrupt); }
    let state = decode_state(input.take::<1>()?[0])?;
    let guards = PublicationGuards {
        owner_epoch: OwnerEpoch::new(input.counter()?).map_err(|_| ControlError::StoreCorrupt)?,
        source_catalog_generation: input.counter()?,
        membership_generation: input.counter()?,
        access_generation: input.counter()?,
        shadow_generation: input.counter()?,
        purge_generation: input.counter()?,
        profile_digest: Blake3Digest32::from_bytes(input.take()?),
    };
    let length = usize::from(u16::from_be_bytes(input.take()?));
    if length == 0 || length > MAX_OPAQUE_REF_BYTES || length != input.0.len() {
        return Err(ControlError::StoreCorrupt);
    }
    let reference = core::str::from_utf8(input.0).map_err(|_| ControlError::StoreCorrupt)?;
    let prepared_manifest_ref = ReceiptRef::new(reference).map_err(|_| ControlError::StoreCorrupt)?;
    Ok(PublicationIntent { publication_intent_id: id, target_epoch: epoch,
        prepared_manifest_ref, owner_source_membership_access_guards: guards, state })
}

struct Input<'a>(&'a [u8]);
impl Input<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        let value = self.0.get(..N).ok_or(ControlError::StoreCorrupt)?
            .try_into().map_err(|_| ControlError::StoreCorrupt)?;
        self.0 = &self.0[N..];
        Ok(value)
    }
    fn counter(&mut self) -> Result<u64, ControlError> { Ok(u64::from_be_bytes(self.take()?)) }
}

fn state_tag(state: PublicationIntentState) -> u8 {
    match state {
        PublicationIntentState::Prepared => 1,
        PublicationIntentState::IntentDurable => 2,
        PublicationIntentState::NewPointsAcknowledged => 3,
        PublicationIntentState::OldPointsClosedAcknowledged => 4,
        PublicationIntentState::ReadbackVerified => 5,
        PublicationIntentState::ControlCommitted => 6,
        PublicationIntentState::Reclaimable => 7,
        PublicationIntentState::Compensating => 8,
        PublicationIntentState::Aborted => 9,
        PublicationIntentState::InvalidationOnlyCommitted => 10,
        PublicationIntentState::PublicationBlocked => 11,
    }
}

fn decode_state(tag: u8) -> Result<PublicationIntentState, ControlError> {
    match tag {
        1 => Ok(PublicationIntentState::Prepared),
        2 => Ok(PublicationIntentState::IntentDurable),
        3 => Ok(PublicationIntentState::NewPointsAcknowledged),
        4 => Ok(PublicationIntentState::OldPointsClosedAcknowledged),
        5 => Ok(PublicationIntentState::ReadbackVerified),
        6 => Ok(PublicationIntentState::ControlCommitted),
        7 => Ok(PublicationIntentState::Reclaimable),
        8 => Ok(PublicationIntentState::Compensating),
        9 => Ok(PublicationIntentState::Aborted),
        10 => Ok(PublicationIntentState::InvalidationOnlyCommitted),
        11 => Ok(PublicationIntentState::PublicationBlocked),
        _ => Err(ControlError::StoreCorrupt),
    }
}

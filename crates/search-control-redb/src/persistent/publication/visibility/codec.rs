//! Strict private codecs for technical refs; no point lists, bodies or JSON identity.
use super::*;
use search_contracts::{MAX_OPAQUE_REF_BYTES, OwnerEpoch};

const STATE_MAGIC: &[u8; 8] = b"ELIVIS01";
const RECEIPT_MAGIC: &[u8; 8] = b"ELIVRC01";
const MANIFEST_MAGIC: &[u8; 8] = b"ELIMRF01";
const SHADOW_MAGIC: &[u8; 8] = b"ELISHW01";

pub(super) fn state(state: &PublicationVisibilityState, limits: JournalLimits) -> Result<ControlValue, ControlError> {
    if (state.visible_epoch.get() == 0) != state.last_receipt.is_none() { return Err(ControlError::InvalidValue); }
    let mut out = STATE_MAGIC.to_vec();
    out.extend_from_slice(state.collection_generation_id.as_bytes());
    out.extend_from_slice(state.schema_identity_digest.as_bytes());
    out.extend_from_slice(&state.visible_epoch.get().to_be_bytes());
    let guards = state.guards;
    for number in [guards.owner_epoch.get(), guards.source_catalog_generation, guards.membership_generation,
        guards.access_generation, guards.shadow_generation, guards.purge_generation] {
        out.extend_from_slice(&number.to_be_bytes());
    }
    out.extend_from_slice(guards.profile_digest.as_bytes());
    match state.last_receipt {
        None => out.push(0),
        Some(id) => { out.push(1); out.extend_from_slice(id.as_bytes()); }
    }
    ControlValue::new(ControlRecordClass::Revision, out, limits)
}

pub(super) fn read_state(value: &ControlValue) -> Result<PublicationVisibilityState, ControlError> {
    let mut input = Input::new(value, ControlRecordClass::Revision, STATE_MAGIC)?;
    let collection_generation_id = CollectionGenerationId::from_bytes(input.take()?);
    let schema_identity_digest = Blake3Digest32::from_bytes(input.take()?);
    let visible_epoch = input.epoch()?;
    let guards = PublicationGuards {
        owner_epoch: OwnerEpoch::new(input.u64()?).map_err(|_| ControlError::StoreCorrupt)?,
        source_catalog_generation: input.u64()?, membership_generation: input.u64()?,
        access_generation: input.u64()?, shadow_generation: input.u64()?, purge_generation: input.u64()?,
        profile_digest: Blake3Digest32::from_bytes(input.take()?),
    };
    let last_receipt = match input.take::<1>()?[0] {
        0 => None, 1 => Some(PublicationReceiptId::from_bytes(input.take()?)),
        _ => return Err(ControlError::StoreCorrupt),
    };
    input.finish()?;
    if (visible_epoch.get() == 0) != last_receipt.is_none() { return Err(ControlError::StoreCorrupt); }
    Ok(PublicationVisibilityState { collection_generation_id, schema_identity_digest, visible_epoch, guards, last_receipt })
}

pub(super) struct BoundReceipt {
    pub operation_id: MutationId,
    pub collection_generation_id: CollectionGenerationId,
    pub schema_identity_digest: Blake3Digest32,
    pub intent: PublicationIntent,
    pub committed_shadow_generation: u64,
    pub publication: PublicationReceipt,
}

pub(super) fn receipt(request: &VisibleEpochCommit, limits: JournalLimits) -> Result<ControlValue, ControlError> {
    let mut out = RECEIPT_MAGIC.to_vec();
    out.extend_from_slice(&request.operation_id.0);
    out.extend_from_slice(request.previous.collection_generation_id.as_bytes());
    out.extend_from_slice(request.previous.schema_identity_digest.as_bytes());
    out.extend_from_slice(&request.previous.visible_epoch.get().to_be_bytes());
    let shadow_generation = request.previous.guards.shadow_generation.checked_add(u64::from(
        request.changes.iter().any(|change| change.matching_shadow.is_some())))
        .ok_or(ControlError::GenerationExhausted)?;
    out.extend_from_slice(&shadow_generation.to_be_bytes());
    let intent = intent_codec::encode(&request.intent, limits)?;
    bytes(&mut out, intent.as_bytes())?;
    let value = &request.receipt;
    out.extend_from_slice(value.publication_receipt_id.as_bytes());
    out.extend_from_slice(&value.target_epoch.get().to_be_bytes());
    reference(&mut out, &value.exact_new_manifest_ref)?;
    reference(&mut out, &value.exact_retired_manifest_ref)?;
    out.extend_from_slice(value.readback_digest.as_bytes());
    out.extend_from_slice(&value.control_commit_revision.get().to_be_bytes());
    ControlValue::new(ControlRecordClass::Receipt, out, limits)
}

pub(super) fn read_receipt(value: &ControlValue) -> Result<BoundReceipt, ControlError> {
    let mut input = Input::new(value, ControlRecordClass::Receipt, RECEIPT_MAGIC)?;
    let operation_id = MutationId(input.take()?);
    let collection_generation_id = CollectionGenerationId::from_bytes(input.take()?);
    let schema_identity_digest = Blake3Digest32::from_bytes(input.take()?);
    let previous_epoch = input.epoch()?;
    let committed_shadow_generation = input.u64()?;
    // Bound before copying the nested value. Its own strict codec checks all fields.
    let nested = input.bytes(128 + MAX_OPAQUE_REF_BYTES)?;
    let nested = ControlValue::new(ControlRecordClass::Operation, nested.to_vec(), JournalLimits::BASELINE)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let intent = intent_codec::decode(&nested)?;
    let publication = PublicationReceipt {
        publication_receipt_id: PublicationReceiptId::from_bytes(input.take()?),
        target_epoch: input.epoch()?, exact_new_manifest_ref: input.reference()?,
        exact_retired_manifest_ref: input.reference()?, readback_digest: Blake3Digest32::from_bytes(input.take()?),
        control_commit_revision: NonZeroRevision::new(input.u64()?).map_err(|_| ControlError::StoreCorrupt)?,
    };
    input.finish()?;
    if intent.state != PublicationIntentState::ReadbackVerified
        || publication.target_epoch != intent.target_epoch
        || previous_epoch.checked_next().ok() != Some(intent.target_epoch)
        || committed_shadow_generation < intent.owner_source_membership_access_guards.shadow_generation
        || committed_shadow_generation.saturating_sub(intent.owner_source_membership_access_guards.shadow_generation) > 1 {
        return Err(ControlError::StoreCorrupt);
    }
    Ok(BoundReceipt { operation_id, collection_generation_id, schema_identity_digest, committed_shadow_generation, intent, publication })
}

pub(super) fn manifest(reference_value: &ReceiptRef, limits: JournalLimits) -> Result<ControlValue, ControlError> {
    let mut out = MANIFEST_MAGIC.to_vec(); reference(&mut out, reference_value)?;
    ControlValue::new(ControlRecordClass::Snapshot, out, limits)
}
pub(super) fn read_manifest(value: &ControlValue) -> Result<ReceiptRef, ControlError> {
    let mut input = Input::new(value, ControlRecordClass::Snapshot, MANIFEST_MAGIC)?;
    let reference = input.reference()?; input.finish()?; Ok(reference)
}
pub(super) fn shadow(value: &PublicationSourceShadow, limits: JournalLimits) -> Result<ControlValue, ControlError> {
    let mut out = SHADOW_MAGIC.to_vec(); out.extend_from_slice(value.source_revision_id.as_bytes());
    out.extend_from_slice(&value.fence_revision.get().to_be_bytes());
    ControlValue::new(ControlRecordClass::State, out, limits)
}
pub(super) fn read_shadow(value: &ControlValue) -> Result<PublicationSourceShadow, ControlError> {
    let mut input = Input::new(value, ControlRecordClass::State, SHADOW_MAGIC)?;
    let value = PublicationSourceShadow { source_revision_id: SourceRevisionId::from_bytes(input.take()?),
        fence_revision: NonZeroRevision::new(input.u64()?).map_err(|_| ControlError::StoreCorrupt)? };
    input.finish()?; Ok(value)
}
fn bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), ControlError> {
    let size = u16::try_from(value.len()).map_err(|_| ControlError::InvalidValue)?;
    out.extend_from_slice(&size.to_be_bytes()); out.extend_from_slice(value); Ok(())
}
fn reference(out: &mut Vec<u8>, value: &ReceiptRef) -> Result<(), ControlError> {
    if value.as_str().is_empty() || value.as_str().len() > MAX_OPAQUE_REF_BYTES { return Err(ControlError::InvalidValue); }
    bytes(out, value.as_str().as_bytes())
}
struct Input<'a>(&'a [u8]);
impl<'a> Input<'a> {
    fn new(value: &'a ControlValue, class: ControlRecordClass, magic: &[u8; 8]) -> Result<Self, ControlError> {
        let mut input = Self(value.as_bytes());
        if value.class() != class || &input.take::<8>()? != magic { return Err(ControlError::StoreCorrupt); }
        Ok(input)
    }
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        let value = self.0.get(..N).ok_or(ControlError::StoreCorrupt)?.try_into().map_err(|_| ControlError::StoreCorrupt)?;
        self.0 = &self.0[N..]; Ok(value)
    }
    fn u64(&mut self) -> Result<u64, ControlError> { Ok(u64::from_be_bytes(self.take()?)) }
    fn epoch(&mut self) -> Result<Epoch, ControlError> {
        Epoch::new(i64::from_be_bytes(self.take()?)).map_err(|_| ControlError::StoreCorrupt)
    }
    fn bytes(&mut self, ceiling: usize) -> Result<&'a [u8], ControlError> {
        let size = usize::from(u16::from_be_bytes(self.take()?));
        if size == 0 || size > ceiling { return Err(ControlError::StoreCorrupt); }
        let value = self.0.get(..size).ok_or(ControlError::StoreCorrupt)?;
        self.0 = &self.0[size..]; Ok(value)
    }
    fn reference(&mut self) -> Result<ReceiptRef, ControlError> {
        let value = core::str::from_utf8(self.bytes(MAX_OPAQUE_REF_BYTES)?).map_err(|_| ControlError::StoreCorrupt)?;
        ReceiptRef::new(value).map_err(|_| ControlError::StoreCorrupt)
    }
    fn finish(self) -> Result<(), ControlError> { if self.0.is_empty() { Ok(()) } else { Err(ControlError::StoreCorrupt) } }
}

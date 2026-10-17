//! Request bounds and coherent snapshot relationships, before exposure or writes.
use super::{Check, ControlError, ControlKey, ControlValue, JournalLimits, JournalReadSnapshot,
    MANIFESTS, PublicationIntentState,
    PublicationVisibilityState, Point, RECEIPTS, RETIRED, ReadTransaction, SHADOWS, STATE,
    VisibleEpochCommit, codec, id_key, intent_codec, key};
use std::collections::BTreeSet;
use super::super::super::{Header, META, map_storage_error, map_table_error};

pub(super) fn validate_request(request: &VisibleEpochCommit, limits: JournalLimits) -> Result<(), ControlError> {
    if request.intent.state != PublicationIntentState::ReadbackVerified
        || request.previous.visible_epoch.checked_next().ok() != Some(request.intent.target_epoch)
        || request.previous.guards != request.intent.owner_source_membership_access_guards
        || request.previous.last_receipt == Some(request.receipt.publication_receipt_id)
        || (request.previous.visible_epoch.get() == 0) != request.previous.last_receipt.is_none()
        || request.prior_commit.before_generation.checked_add(1) != Some(request.prior_commit.after_generation)
        || request.prior_commit.operation_id == request.operation_id
        || !request.prior_commit.changed_keys.iter().any(|key| key.as_bytes() == super::super::KEY)
        || request.changes.is_empty() {
        return Err(ControlError::InvalidValue);
    }
    if request.prior_commit.changed_keys.len() > limits.max_mutation_items
        || request.prior_commit.changed_keys.iter().any(|key| key.as_bytes().len() > limits.max_key_bytes)
        || request.prior_commit.changed_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ControlError::InvalidValue);
    }
    // Three core writes and three core guards; worst case per membership:
    // pointer write/delete + retirement + shadow delete + three conditions.
    let mut work = 6_usize;
    let mut members = BTreeSet::new();
    if request.changes.len() > limits.max_mutation_items { return Err(ControlError::BudgetExceeded); }
    for change in &request.changes {
        if !members.insert(change.membership_id) { return Err(ControlError::DuplicateMutationKey); }
        if change.previous_manifest == change.next_manifest {
            return Err(ControlError::InvalidValue);
        }
        work = work.checked_add(3 + usize::from(change.previous_manifest.is_some()) * 2
            + usize::from(change.matching_shadow.is_some())).ok_or(ControlError::BudgetExceeded)?;
    }
    if work > limits.max_mutation_items { return Err(ControlError::BudgetExceeded); }
    if request.changes.iter().any(|change| change.matching_shadow.is_some())
        && request.previous.guards.shadow_generation == u64::MAX {
        return Err(ControlError::GenerationExhausted);
    }
    Ok(())
}

pub(in crate::persistent) fn validate_record(key: &ControlKey, value: &ControlValue) -> Result<(), ControlError> {
    let bytes = key.as_bytes();
    if bytes == STATE { codec::read_state(value)?; }
    else if let Some(id) = bytes.strip_prefix(RECEIPTS) {
        let record = codec::read_receipt(value)?;
        if id != record.publication.publication_receipt_id.as_bytes() { return Err(ControlError::StoreCorrupt); }
    } else if let Some(id) = bytes.strip_prefix(MANIFESTS) {
        if id.len() != 16 { return Err(ControlError::StoreCorrupt); }
        codec::read_manifest(value)?;
    } else if let Some(id) = bytes.strip_prefix(SHADOWS) {
        if id.len() != 16 { return Err(ControlError::StoreCorrupt); }
        codec::read_shadow(value)?;
    } else if let Some(ids) = bytes.strip_prefix(RETIRED) {
        if ids.len() != 32 { return Err(ControlError::StoreCorrupt); }
        codec::read_manifest(value)?;
    }
    Ok(())
}

fn validate_abort_operation(
    read: &ReadTransaction,
    snapshot: &JournalReadSnapshot,
    record: &super::super::abort_finalization::PublicationAbortFinalizationRecordV1,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let metadata = record.metadata();
    let meta = read.open_table(META).map_err(map_table_error)?;
    let header_bytes = meta
        .get("header")
        .map_err(|error| map_storage_error(&error))?
        .ok_or(ControlError::StoreCorrupt)?;
    check.reserve(crate::MigrationBootstrapWork::Read(header_bytes.value().len()))?;
    check.reserve(crate::MigrationBootstrapWork::Process(header_bytes.value().len()))?;
    let header = Header::decode(header_bytes.value(), snapshot.identity, limits)?;
    let operation = super::super::super::operation_from_checked(
        read,
        metadata.c02_mutation_id,
        &header,
        limits,
        check,
    )?
    .ok_or(ControlError::StoreCorrupt)?;
    let receipt = operation.receipt;
    let abort_key = super::super::abort_finalization::key(limits)?;
    if receipt.command_digest != metadata.c02_command_digest
        || receipt.before_generation != metadata.expected_control_generation
        || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
        || !receipt.changed_keys.contains(&abort_key)
        || !receipt.changed_keys.contains(&key(super::super::KEY, limits)?)
    {
        return Err(ControlError::StoreCorrupt);
    }
    Ok(())
}
pub(in crate::persistent) fn validate_snapshot(read: &ReadTransaction, snapshot: &JournalReadSnapshot,
    limits: JournalLimits, check: &dyn Check)
    -> Result<Option<PublicationVisibilityState>, ControlError> {
    if !crate::has_visibility_schema(snapshot.identity.schema_version) { return Err(ControlError::SchemaUnsupported); }
    check.check(Point::ReadRecord)?;
    let state = match find(snapshot, STATE) {
        Some(value) => codec::read_state(value)?,
        None if snapshot.generation == 0 && snapshot.records.is_empty() => return Ok(None),
        None => return Err(ControlError::StoreCorrupt),
    };
    if state.guards.owner_epoch > snapshot.identity.owner_epoch { return Err(ControlError::StoreCorrupt); }
    let intent = find(snapshot, super::super::KEY).map(intent_codec::decode).transpose()?;
    let original_binding =
        super::super::operation_binding::read_snapshot(snapshot, limits, check)?;
    let abort_finalization =
        super::super::abort_finalization::read_snapshot(snapshot, limits, check)?;
    if original_binding.as_ref().is_some_and(|record| {
        intent.as_ref().is_none_or(|intent| !record.matches_intent(intent))
    }) || abort_finalization.is_some() && original_binding.is_none()
    {
        return Err(ControlError::StoreCorrupt);
    }
    if abort_finalization.as_ref().is_some_and(|record| {
        intent.as_ref().is_none_or(|intent| !record.matches_intent(intent))
    }) {
        return Err(ControlError::StoreCorrupt);
    }
    validate_retained_abandonment_fence(
        read,
        snapshot,
        &state,
        intent.as_ref(),
        abort_finalization.as_ref(),
        limits,
        check,
    )?;
    if let Some(intent) = &intent {
        super::super::prepared_artifacts::validate_reference(snapshot, &intent.prepared_manifest_ref, limits, check)?;
    }
    if let Some(record) = &abort_finalization {
        validate_abort_operation(read, snapshot, record, limits, check)?;
    }
    let receipt = validate_current_receipt(read, snapshot, &state, limits, check)?;
    match intent {
        None if state.visible_epoch.get() != 0 => return Err(ControlError::StoreCorrupt),
        Some(intent) if matches!(intent.state, PublicationIntentState::ControlCommitted | PublicationIntentState::Reclaimable) => {
            let receipt = receipt.ok_or(ControlError::StoreCorrupt)?;
            let mut expected = receipt.intent;
            expected.state = intent.state;
            // Two separate checks preserve quarantine-on-mismatch: neither a
            // substituted intent nor a skewed epoch may pass.
            if expected != intent { return Err(ControlError::StoreCorrupt); }
            if intent.target_epoch != state.visible_epoch { return Err(ControlError::StoreCorrupt); }
        }
        Some(intent) => {
            if intent.target_epoch <= state.visible_epoch { return Err(ControlError::StoreCorrupt); }
            // Invalidation-only and skipped epochs require their separate complete protocol.
            if intent.state == PublicationIntentState::InvalidationOnlyCommitted { return Err(ControlError::StoreCorrupt); }
        }
        None => {}
    }
    // Every encoded publication reference in this bounded snapshot must validate.
    // This is explicit consistency verification, not a query-history write.
    for (key, value) in &snapshot.records {
        check.check(Point::ReadRecord)?; validate_record(key, value)?;
        if key.as_bytes().starts_with(RECEIPTS) {
            let recorded = codec::read_receipt(value)?;
            super::super::prepared_artifacts::validate_reference(snapshot, &recorded.intent.prepared_manifest_ref, limits, check)?;
            if recorded.publication.target_epoch > state.visible_epoch
                || recorded.publication.control_commit_revision.get() > snapshot.generation {
                return Err(ControlError::StoreCorrupt);
            }
        }
        if key.as_bytes().starts_with(MANIFESTS) || key.as_bytes().starts_with(RETIRED) {
            super::super::prepared_artifacts::validate_reference(snapshot, &codec::read_manifest(value)?, limits, check)?;
        }
        if let Some(ids) = key.as_bytes().strip_prefix(RETIRED) {
            let id: [u8; 16] = ids[..16].try_into().map_err(|_| ControlError::StoreCorrupt)?;
            if snapshot.get(&id_key(RECEIPTS, &id, limits)?).is_none() { return Err(ControlError::StoreCorrupt); }
        }
    }
    check.check(Point::ReadComplete)?;
    Ok(Some(state))
}

fn validate_current_receipt(
    read: &ReadTransaction,
    snapshot: &JournalReadSnapshot,
    state: &PublicationVisibilityState,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<Option<codec::BoundReceipt>, ControlError> {
    let Some(id) = state.last_receipt else {
        return Ok(None);
    };
    let receipt_key = id_key(RECEIPTS, id.as_bytes(), limits)?;
    let value = snapshot.get(&receipt_key).ok_or(ControlError::StoreCorrupt)?;
    let receipt = codec::read_receipt(value)?;
    if receipt.publication.publication_receipt_id != id
        || receipt.collection_generation_id != state.collection_generation_id
        || receipt.schema_identity_digest != state.schema_identity_digest
        || receipt.publication.target_epoch != state.visible_epoch
        || receipt.publication.control_commit_revision.get() > snapshot.generation
    {
        return Err(ControlError::StoreCorrupt);
    }
    // Bind the typed receipt to an actual committed operation in this same read.
    let meta = read.open_table(META).map_err(map_table_error)?;
    let header_bytes = meta
        .get("header")
        .map_err(|error| map_storage_error(&error))?
        .ok_or(ControlError::StoreCorrupt)?;
    check.reserve(crate::MigrationBootstrapWork::Read(header_bytes.value().len()))?;
    check.reserve(crate::MigrationBootstrapWork::Process(header_bytes.value().len()))?;
    let header = Header::decode(header_bytes.value(), snapshot.identity, limits)?;
    let operation = super::super::super::operation_from_checked(
        read,
        receipt.operation_id,
        &header,
        limits,
        check,
    )?
    .ok_or(ControlError::StoreCorrupt)?;
    if operation.receipt.after_generation != receipt.publication.control_commit_revision.get()
        || !operation.receipt.changed_keys.contains(&receipt_key)
        || !operation.receipt.changed_keys.contains(&key(STATE, limits)?)
        || !operation
            .receipt
            .changed_keys
            .contains(&key(super::super::KEY, limits)?)
        || receipt
            .intent
            .owner_source_membership_access_guards
            .profile_digest
            != state.guards.profile_digest
    {
        return Err(ControlError::StoreCorrupt);
    }
    if receipt.publication.control_commit_revision.get() == snapshot.generation {
        let mut expected_guards = receipt.intent.owner_source_membership_access_guards;
        expected_guards.shadow_generation = receipt.committed_shadow_generation;
        if state.guards != expected_guards {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(Some(receipt))
}

fn validate_retained_abandonment_fence(
    read: &ReadTransaction,
    snapshot: &JournalReadSnapshot,
    visibility: &PublicationVisibilityState,
    intent: Option<&search_contracts::PublicationIntent>,
    finalization: Option<&super::super::PublicationAbortFinalizationRecordV1>,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let Some(fence) = super::super::abandonment::read_snapshot(snapshot, limits, check)? else {
        return Ok(());
    };
    super::super::abandonment::validate_retained_fence(
        read,
        snapshot,
        visibility,
        intent.ok_or(ControlError::StoreCorrupt)?,
        finalization,
        &fence,
        limits,
        check,
    )
}

fn find<'a>(snapshot: &'a JournalReadSnapshot, key: &[u8]) -> Option<&'a ControlValue> {
    snapshot.records.binary_search_by(|(candidate, _)| candidate.as_bytes().cmp(key))
        .ok().map(|index| &snapshot.records[index].1)
}

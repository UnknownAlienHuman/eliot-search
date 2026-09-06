//! Request bounds and coherent snapshot relationships, before exposure or writes.
use super::*;
use std::collections::BTreeSet;
use super::super::super::{Header, META, ReadableTable, map_storage_error, map_table_error};

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

pub(in crate::persistent) fn validate_snapshot(read: &ReadTransaction, snapshot: &JournalReadSnapshot,
    limits: JournalLimits, check: &dyn Check)
    -> Result<Option<PublicationVisibilityState>, ControlError> {
    if snapshot.identity.schema_version != PUBLICATION_VISIBILITY_SCHEMA_VERSION { return Err(ControlError::SchemaUnsupported); }
    check.check(Point::ReadRecord)?;
    let state = match find(snapshot, STATE) {
        Some(value) => codec::read_state(value)?,
        None if snapshot.generation == 0 && snapshot.records.is_empty() => return Ok(None),
        None => return Err(ControlError::StoreCorrupt),
    };
    if state.guards.owner_epoch > snapshot.identity.owner_epoch { return Err(ControlError::StoreCorrupt); }
    let intent = find(snapshot, super::super::KEY).map(intent_codec::decode).transpose()?;
    let receipt = match state.last_receipt {
        None => None,
        Some(id) => {
            let receipt_key = id_key(RECEIPTS, id.as_bytes(), limits)?;
            let value = snapshot.get(&receipt_key).ok_or(ControlError::StoreCorrupt)?;
            let receipt = codec::read_receipt(value)?;
            if receipt.publication.publication_receipt_id != id
                || receipt.collection_generation_id != state.collection_generation_id
                || receipt.schema_identity_digest != state.schema_identity_digest
                || receipt.publication.target_epoch != state.visible_epoch
                || receipt.publication.control_commit_revision.get() > snapshot.generation {
                return Err(ControlError::StoreCorrupt);
            }
            // Tie the typed receipt to an actual committed operation in the same
            // read transaction, not only to a caller-populated revision number.
            let meta = read.open_table(META).map_err(map_table_error)?;
            let header_bytes = meta.get("header").map_err(map_storage_error)?.ok_or(ControlError::StoreCorrupt)?;
            let header = Header::decode(header_bytes.value(), snapshot.identity, limits)?;
            let operation = operation_from(read, receipt.operation_id, &header, limits)?
                .ok_or(ControlError::StoreCorrupt)?;
            if operation.receipt.after_generation != receipt.publication.control_commit_revision.get()
                || !operation.receipt.changed_keys.contains(&receipt_key)
                || !operation.receipt.changed_keys.contains(&key(STATE, limits)?)
                || !operation.receipt.changed_keys.contains(&key(super::super::KEY, limits)?)
                || receipt.intent.owner_source_membership_access_guards.profile_digest != state.guards.profile_digest {
                return Err(ControlError::StoreCorrupt);
            }
            if receipt.publication.control_commit_revision.get() == snapshot.generation {
                let mut expected_guards = receipt.intent.owner_source_membership_access_guards;
                expected_guards.shadow_generation = receipt.committed_shadow_generation;
                if state.guards != expected_guards { return Err(ControlError::StoreCorrupt); }
            }
            Some(receipt)
        }
    };
    match intent {
        None if state.visible_epoch.get() != 0 => return Err(ControlError::StoreCorrupt),
        Some(intent) if matches!(intent.state, PublicationIntentState::ControlCommitted | PublicationIntentState::Reclaimable) => {
            let receipt = receipt.ok_or(ControlError::StoreCorrupt)?;
            let mut expected = receipt.intent;
            expected.state = intent.state;
            if expected != intent || intent.target_epoch != state.visible_epoch { return Err(ControlError::StoreCorrupt); }
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
            if recorded.publication.target_epoch > state.visible_epoch
                || recorded.publication.control_commit_revision.get() > snapshot.generation {
                return Err(ControlError::StoreCorrupt);
            }
        }
        if let Some(ids) = key.as_bytes().strip_prefix(RETIRED) {
            let id: [u8; 16] = ids[..16].try_into().map_err(|_| ControlError::StoreCorrupt)?;
            if snapshot.get(&id_key(RECEIPTS, &id, limits)?).is_none() { return Err(ControlError::StoreCorrupt); }
        }
    }
    check.check(Point::ReadComplete)?;
    Ok(Some(state))
}

fn find<'a>(snapshot: &'a JournalReadSnapshot, key: &[u8]) -> Option<&'a ControlValue> {
    snapshot.records.binary_search_by(|(candidate, _)| candidate.as_bytes().cmp(key))
        .ok().map(|index| &snapshot.records[index].1)
}

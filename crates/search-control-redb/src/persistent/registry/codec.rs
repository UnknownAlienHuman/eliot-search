//! Storage wrappers only. Canonical source-owner payloads remain opaque.
use super::{
    PREFIX, REVISION, RegistryOperationEnvelope, RegistryStateFamily, RegistryStateKey,
    RegistryStoredState,
};
use crate::{
    ControlError, ControlKey, ControlRecordClass, ControlValue, JournalLimits, MutationId,
};
use search_contracts::{Blake3Digest32, OpaqueId};
use sha2::{Digest, Sha256};

const STATE: &[u8] = b"eliot.control.registry.v1/state/";
const OPERATION: &[u8] = b"eliot.control.registry.v1/operation/";
const STATE_MAGIC: &[u8; 8] = b"ELRGST01";
const OPERATION_MAGIC: &[u8; 8] = b"ELRGOP01";
const REVISION_MAGIC: &[u8; 8] = b"ELRGREV1";

pub(super) fn operation_size(envelope: &RegistryOperationEnvelope) -> Result<usize, ControlError> {
    operation_value_len(&envelope.operation_id, envelope.owner_bytes.len())
}

pub(super) fn operation_value_len(
    operation_id: &OpaqueId,
    owner_bytes_len: usize,
) -> Result<usize, ControlError> {
    let id_len = operation_id.as_str().len();
    if id_len == 0 || owner_bytes_len == 0 {
        return Err(ControlError::InvalidValue);
    }
    u32::try_from(id_len).map_err(|_| ControlError::BudgetExceeded)?;
    u32::try_from(owner_bytes_len).map_err(|_| ControlError::BudgetExceeded)?;
    64_usize
        .checked_add(id_len)
        .and_then(|n| n.checked_add(owner_bytes_len))
        .ok_or(ControlError::BudgetExceeded)
}

pub(super) fn key_size(key: &RegistryStateKey) -> Result<usize, ControlError> {
    key.bytes.len().checked_add(STATE.len() + 5).ok_or(ControlError::BudgetExceeded)
}

pub(super) fn decode_record_checked(key: &ControlKey, value: &ControlValue, limits: JournalLimits,
    check: &dyn super::super::Check,
) -> Result<Record, ControlError> {
    use super::super::MigrationBootstrapWork as Work;
    let bytes = value.len().checked_mul(2).and_then(|n| n.checked_add(key.as_bytes().len()))
        .ok_or(ControlError::BudgetExceeded)?;
    // Covers owned decoded fields and the existing codec's canonical re-encode.
    check.reserve(Work::Allocate(bytes))?;
    check.reserve(Work::Process(bytes))?;
    decode_record(key, value, limits)
}

pub(super) enum Record {
    Revision(u64),
    State(RegistryStoredState),
    Operation(RegistryOperationEnvelope),
}

pub(super) fn mutation_id(id: &OpaqueId) -> MutationId {
    // Technical indexing only. Keep and compare the complete raw ID in the row.
    // This SHA-256 never populates a BLAKE3 field or claims source identity.
    let mut hash = Sha256::new();
    hash.update(b"eliot-search/registry-operation-index/sha256/v1\0");
    hash.update(id.as_str().as_bytes());
    MutationId(hash.finalize().into())
}

pub(super) fn operation_key(
    id: &OpaqueId,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    let mut bytes = OPERATION.to_vec();
    bytes.extend_from_slice(&mutation_id(id).0);
    ControlKey::new(bytes, limits)
}

pub(super) fn state_key(
    key: &RegistryStateKey,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    if key.bytes.is_empty() {
        return Err(ControlError::InvalidKey);
    }
    if key
        .bytes
        .len()
        .checked_add(STATE.len() + 5)
        .is_none_or(|n| n > limits.max_key_bytes)
    {
        return Err(ControlError::BudgetExceeded);
    }
    let mut out = STATE.to_vec();
    out.push(key.family.tag());
    put_bytes(&mut out, &key.bytes)?;
    ControlKey::new(out, limits)
}

pub(super) fn revision_value(
    revision: u64,
    limits: JournalLimits,
) -> Result<ControlValue, ControlError> {
    let mut bytes = REVISION_MAGIC.to_vec();
    bytes.extend_from_slice(&revision.to_be_bytes());
    ControlValue::new(ControlRecordClass::Revision, bytes, limits)
}

pub(super) fn state_value(
    family: RegistryStateFamily,
    bytes: &[u8],
    limits: JournalLimits,
) -> Result<ControlValue, ControlError> {
    bounded(bytes, 12, limits)?;
    let mut out = STATE_MAGIC.to_vec();
    put_bytes(&mut out, bytes)?;
    ControlValue::new(family.class(), out, limits)
}

pub(super) fn operation_value(
    envelope: &RegistryOperationEnvelope,
    limits: JournalLimits,
) -> Result<ControlValue, ControlError> {
    bounded(
        &envelope.owner_bytes,
        64_usize
            .checked_add(envelope.operation_id.as_str().len())
            .ok_or(ControlError::BudgetExceeded)?,
        limits,
    )?;
    let mut out = OPERATION_MAGIC.to_vec();
    out.extend_from_slice(&envelope.journal_generation.to_be_bytes());
    out.extend_from_slice(&envelope.registry_revision.to_be_bytes());
    out.extend_from_slice(envelope.command_digest.as_bytes());
    put_bytes(&mut out, envelope.operation_id.as_str().as_bytes())?;
    put_bytes(&mut out, &envelope.owner_bytes)?;
    ControlValue::new(ControlRecordClass::Operation, out, limits)
}

fn bounded(bytes: &[u8], overhead: usize, limits: JournalLimits) -> Result<(), ControlError> {
    if bytes.is_empty() {
        return Err(ControlError::InvalidValue);
    }
    if bytes
        .len()
        .checked_add(overhead)
        .is_none_or(|n| n > limits.max_value_bytes)
    {
        return Err(ControlError::BudgetExceeded);
    }
    Ok(())
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ControlError> {
    out.extend_from_slice(
        &u32::try_from(bytes.len())
            .map_err(|_| ControlError::BudgetExceeded)?
            .to_be_bytes(),
    );
    out.extend_from_slice(bytes);
    Ok(())
}

pub(super) fn decode_record(
    key: &ControlKey,
    value: &ControlValue,
    limits: JournalLimits,
) -> Result<Record, ControlError> {
    if value.len() > limits.max_value_bytes {
        return Err(ControlError::StoreCorrupt);
    }
    let raw = key.as_bytes();
    if raw == REVISION {
        if value.class() != ControlRecordClass::Revision {
            return Err(ControlError::StoreCorrupt);
        }
        let mut input = Input(value.as_bytes());
        input.magic(REVISION_MAGIC)?;
        let revision = input.u64()?;
        input.finish()?;
        return Ok(Record::Revision(revision));
    }
    if let Some(suffix) = raw.strip_prefix(STATE) {
        let mut input = Input(suffix);
        let family = match input.take(1)?[0] {
            1 => RegistryStateFamily::Source,
            2 => RegistryStateFamily::Membership,
            3 => RegistryStateFamily::Generation,
            4 => RegistryStateFamily::Root,
            5 => RegistryStateFamily::Namespace,
            _ => return Err(ControlError::StoreCorrupt),
        };
        let owner_key = input.bytes()?.to_vec();
        input.finish()?;
        let state_key = RegistryStateKey::new(family, owner_key, limits)?;
        if state_key.family.class() != value.class() {
            return Err(ControlError::StoreCorrupt);
        }
        let mut input = Input(value.as_bytes());
        input.magic(STATE_MAGIC)?;
        let bytes = input.bytes()?.to_vec();
        input.finish()?;
        state_value(family, &bytes, limits)?;
        return Ok(Record::State(RegistryStoredState {
            key: state_key,
            bytes,
        }));
    }
    if raw.starts_with(OPERATION) {
        if value.class() != ControlRecordClass::Operation {
            return Err(ControlError::StoreCorrupt);
        }
        let mut input = Input(value.as_bytes());
        input.magic(OPERATION_MAGIC)?;
        let journal_generation = input.u64()?;
        let registry_revision = input.u64()?;
        journal_generation
            .checked_add(1)
            .ok_or(ControlError::StoreCorrupt)?;
        registry_revision
            .checked_add(1)
            .ok_or(ControlError::StoreCorrupt)?;
        let command_digest = Blake3Digest32::from_stored_bytes(
            input
                .take(32)?
                .try_into()
                .map_err(|_| ControlError::StoreCorrupt)?,
        );
        let operation_id = OpaqueId::new(
            std::str::from_utf8(input.bytes()?)
                .map_err(|_| ControlError::StoreCorrupt)?
                .to_owned(),
        )
        .map_err(|_| ControlError::StoreCorrupt)?;
        let owner_bytes = input.bytes()?.to_vec();
        input.finish()?;
        let envelope = RegistryOperationEnvelope {
            operation_id,
            journal_generation,
            registry_revision,
            command_digest,
            owner_bytes,
        };
        if operation_key(&envelope.operation_id, limits)? != *key {
            return Err(ControlError::OperationConflict);
        }
        operation_value(&envelope, limits)?;
        return Ok(Record::Operation(envelope));
    }
    if raw.starts_with(PREFIX) {
        return Err(ControlError::StoreCorrupt);
    }
    Err(ControlError::InvalidKey)
}

struct Input<'a>(&'a [u8]);
impl<'a> Input<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ControlError> {
        let bytes = self.0.get(..n).ok_or(ControlError::StoreCorrupt)?;
        self.0 = &self.0[n..];
        Ok(bytes)
    }
    fn magic(&mut self, expected: &[u8]) -> Result<(), ControlError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(ControlError::StoreCorrupt)
        }
    }
    fn u64(&mut self) -> Result<u64, ControlError> {
        Ok(u64::from_be_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| ControlError::StoreCorrupt)?,
        ))
    }
    fn bytes(&mut self) -> Result<&'a [u8], ControlError> {
        let n = u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| ControlError::StoreCorrupt)?,
        );
        self.take(usize::try_from(n).map_err(|_| ControlError::StoreCorrupt)?)
    }
    const fn finish(self) -> Result<(), ControlError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(ControlError::StoreCorrupt)
        }
    }
}

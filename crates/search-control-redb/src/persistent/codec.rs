//! Strict private disk codecs. SHA-256 request binding is not a BLAKE3 alias.

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch};
use sha2::{Digest, Sha256};

use crate::{ControlCommitReceipt, ControlError, ControlKey, ControlMutation, ControlRecordClass,
    ControlValue, JournalIdentity, JournalLimits, MutationId};

const HEADER_MAGIC: &[u8; 8] = b"ELCTRL01";
const RECEIPT_MAGIC: &[u8; 8] = b"ELCTOP01";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Header {
    pub identity: JournalIdentity,
    pub generation: u64,
    pub records: u64,
    pub value_bytes: u64,
    pub operations: u64,
    pub operation_bytes: u64,
}

impl Header {
    pub fn empty(identity: JournalIdentity) -> Self {
        Self { identity, generation: 0, records: 0, value_bytes: 0, operations: 0, operation_bytes: 0 }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = HEADER_MAGIC.to_vec();
        out.extend(identity_bytes(self.identity));
        for value in [self.generation, self.records, self.value_bytes, self.operations, self.operation_bytes] {
            out.extend_from_slice(&value.to_be_bytes());
        }
        out
    }

    pub fn decode(bytes: &[u8], expected: JournalIdentity, limits: JournalLimits) -> Result<Self, ControlError> {
        let mut input = Decoder(bytes);
        if input.fixed::<8>()? != *HEADER_MAGIC { return Err(ControlError::SchemaUnsupported); }
        let identity = JournalIdentity {
            installation_incarnation_id: InstallationIncarnationId::from_bytes(input.fixed()?),
            data_root_id: DataRootId::from_bytes(input.fixed()?),
            owner_epoch: OwnerEpoch::new(input.u64()?).map_err(|_| ControlError::StoreCorrupt)?,
            path_identity_digest: Blake3Digest32::from_bytes(input.fixed()?),
            schema_family_digest: Blake3Digest32::from_bytes(input.fixed()?),
            schema_version: u32::from_be_bytes(input.fixed()?),
        };
        if identity.schema_version != expected.schema_version || identity.schema_family_digest != expected.schema_family_digest {
            return Err(ControlError::SchemaMismatch);
        }
        if identity != expected { return Err(ControlError::IdentityMismatch); }
        let result = Self {
            identity, generation: input.u64()?, records: input.u64()?, value_bytes: input.u64()?,
            operations: input.u64()?, operation_bytes: input.u64()?,
        };
        input.finish()?;
        // No pruning is supported by this disk format. Every generation has one receipt.
        if (result.generation == 0 && (result.records != 0 || result.value_bytes != 0 || result.operation_bytes != 0))
            || result.generation != result.operations
            || result.records > as_u64(limits.max_records)?
            || result.value_bytes > as_u64(limits.max_total_value_bytes)?
            || result.operations > as_u64(limits.max_operation_records)?
            || result.operation_bytes > as_u64(limits.max_total_value_bytes)?
        { return Err(ControlError::StoreCorrupt); }
        Ok(result)
    }
}

pub(super) fn identity_bytes(identity: JournalIdentity) -> Vec<u8> {
    let mut out = Vec::with_capacity(108);
    out.extend_from_slice(identity.installation_incarnation_id.as_bytes());
    out.extend_from_slice(identity.data_root_id.as_bytes());
    out.extend_from_slice(&identity.owner_epoch.get().to_be_bytes());
    out.extend_from_slice(identity.path_identity_digest.as_bytes());
    out.extend_from_slice(identity.schema_family_digest.as_bytes());
    out.extend_from_slice(&identity.schema_version.to_be_bytes());
    out
}

pub(super) fn as_u64(value: usize) -> Result<u64, ControlError> {
    u64::try_from(value).map_err(|_| ControlError::BudgetExceeded)
}

pub(super) fn class_tag(class: ControlRecordClass) -> u8 {
    match class {
        ControlRecordClass::Identity => 1, ControlRecordClass::Revision => 2,
        ControlRecordClass::State => 3, ControlRecordClass::Receipt => 4,
        ControlRecordClass::Operation => 5, ControlRecordClass::Snapshot => 6,
        ControlRecordClass::Migration => 7,
    }
}

pub(super) fn encode_value(value: &ControlValue) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 1);
    out.push(class_tag(value.class()));
    out.extend_from_slice(value.as_bytes());
    out
}

pub(super) fn decode_value(bytes: &[u8], limits: JournalLimits) -> Result<ControlValue, ControlError> {
    let Some((&tag, value)) = bytes.split_first() else { return Err(ControlError::StoreCorrupt); };
    let class = match tag {
        1 => ControlRecordClass::Identity, 2 => ControlRecordClass::Revision,
        3 => ControlRecordClass::State, 4 => ControlRecordClass::Receipt,
        5 => ControlRecordClass::Operation, 6 => ControlRecordClass::Snapshot,
        7 => ControlRecordClass::Migration, _ => return Err(ControlError::ForbiddenControlPayload),
    };
    if value.is_empty() || value.len() > limits.max_value_bytes { return Err(ControlError::StoreCorrupt); }
    ControlValue::new(class, value.to_vec(), limits)
}

pub(super) fn validate_mutation(mutation: &ControlMutation, limits: JournalLimits) -> Result<Vec<ControlKey>, ControlError> {
    let count = mutation.writes().len().checked_add(mutation.deletes().len()).ok_or(ControlError::BudgetExceeded)?;
    if count == 0 || count > limits.max_mutation_items { return Err(ControlError::BudgetExceeded); }
    let mut keys = BTreeSet::new();
    for write in mutation.writes() {
        validate_key(&write.key, limits)?;
        if write.value.is_empty() || write.value.len() > limits.max_value_bytes { return Err(ControlError::InvalidValue); }
        if !keys.insert(write.key.clone()) { return Err(ControlError::DuplicateMutationKey); }
    }
    for key in mutation.deletes() {
        validate_key(key, limits)?;
        if !keys.insert(key.clone()) { return Err(ControlError::DuplicateMutationKey); }
    }
    Ok(keys.into_iter().collect())
}

fn validate_key(key: &ControlKey, limits: JournalLimits) -> Result<(), ControlError> {
    if key.as_bytes().is_empty() || key.as_bytes().len() > limits.max_key_bytes { Err(ControlError::InvalidKey) } else { Ok(()) }
}

/// Bind actual canonical input, independently of the caller's declared command digest.
pub(super) fn request_fingerprint(identity: JournalIdentity, mutation: &ControlMutation) -> Result<[u8; 32], ControlError> {
    let mut hash = Sha256::new();
    hash.update(b"eliot-search/control-request/sha256/v1\0");
    // The live owner epoch is fenced by the journal header, not by the
    // immutable request. Recovery must survive an explicit owner handoff.
    hash.update(identity.installation_incarnation_id.as_bytes());
    hash.update(identity.data_root_id.as_bytes());
    hash.update(identity.path_identity_digest.as_bytes());
    hash.update(identity.schema_family_digest.as_bytes());
    hash.update(identity.schema_version.to_be_bytes());
    hash.update(mutation.id().0);
    hash.update(mutation.command_digest().as_bytes());
    hash.update(mutation.expected_generation().to_be_bytes());
    let mut writes = mutation.writes().iter().collect::<Vec<_>>();
    writes.sort_by(|a, b| a.key.cmp(&b.key));
    hash.update(as_u64(writes.len())?.to_be_bytes());
    for write in writes {
        hash.update(as_u64(write.key.as_bytes().len())?.to_be_bytes());
        hash.update(write.key.as_bytes());
        hash.update([class_tag(write.value.class())]);
        hash.update(as_u64(write.value.len())?.to_be_bytes());
        hash.update(write.value.as_bytes());
    }
    let mut deletes = mutation.deletes().iter().collect::<Vec<_>>();
    deletes.sort();
    hash.update(as_u64(deletes.len())?.to_be_bytes());
    for key in deletes {
        hash.update(as_u64(key.as_bytes().len())?.to_be_bytes());
        hash.update(key.as_bytes());
    }
    Ok(hash.finalize().into())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StoredOperation {
    pub request_sha256: [u8; 32],
    pub receipt: ControlCommitReceipt,
}

impl StoredOperation {
    pub fn encode(&self, limits: JournalLimits) -> Result<Vec<u8>, ControlError> {
        let mut size = 8_usize + 32 + 32 + 32 + 8 + 8 + 8;
        for key in &self.receipt.changed_keys {
            size = size.checked_add(8).and_then(|n| n.checked_add(key.as_bytes().len())).ok_or(ControlError::BudgetExceeded)?;
        }
        // Receipt entries and their total ledger each have finite byte ceilings.
        if size > limits.max_value_bytes { return Err(ControlError::BudgetExceeded); }
        let mut out = Vec::with_capacity(size);
        out.extend_from_slice(RECEIPT_MAGIC);
        out.extend_from_slice(&self.request_sha256);
        out.extend_from_slice(&self.receipt.operation_id.0);
        out.extend_from_slice(self.receipt.command_digest.as_bytes());
        out.extend_from_slice(&self.receipt.before_generation.to_be_bytes());
        out.extend_from_slice(&self.receipt.after_generation.to_be_bytes());
        out.extend_from_slice(&as_u64(self.receipt.changed_keys.len())?.to_be_bytes());
        for key in &self.receipt.changed_keys {
            out.extend_from_slice(&as_u64(key.as_bytes().len())?.to_be_bytes());
            out.extend_from_slice(key.as_bytes());
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8], id: MutationId, generation: u64, limits: JournalLimits) -> Result<Self, ControlError> {
        if bytes.len() > limits.max_value_bytes { return Err(ControlError::StoreCorrupt); }
        let mut input = Decoder(bytes);
        if input.fixed::<8>()? != *RECEIPT_MAGIC { return Err(ControlError::SchemaMismatch); }
        let request_sha256 = input.fixed()?;
        let operation_id = MutationId(input.fixed()?);
        let command_digest = Blake3Digest32::from_bytes(input.fixed()?);
        let before_generation = input.u64()?;
        let after_generation = input.u64()?;
        let count = usize::try_from(input.u64()?).map_err(|_| ControlError::StoreCorrupt)?;
        if operation_id != id || before_generation.checked_add(1) != Some(after_generation)
            || after_generation > generation || count == 0 || count > limits.max_mutation_items
        { return Err(ControlError::StoreCorrupt); }
        let mut changed_keys = Vec::new();
        for _ in 0..count {
            let len = usize::try_from(input.u64()?).map_err(|_| ControlError::StoreCorrupt)?;
            if len == 0 || len > limits.max_key_bytes { return Err(ControlError::StoreCorrupt); }
            let key = ControlKey::new(input.take(len)?.to_vec(), limits).map_err(|_| ControlError::StoreCorrupt)?;
            if changed_keys.last().is_some_and(|previous| previous >= &key) { return Err(ControlError::StoreCorrupt); }
            changed_keys.push(key);
        }
        input.finish()?;
        Ok(Self { request_sha256, receipt: ControlCommitReceipt {
            operation_id, command_digest, before_generation, after_generation, changed_keys, replayed: false,
        } })
    }
}

struct Decoder<'a>(&'a [u8]);
impl<'a> Decoder<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], ControlError> {
        let value = self.0.get(..length).ok_or(ControlError::StoreCorrupt)?;
        self.0 = &self.0[length..];
        Ok(value)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        self.take(N)?.try_into().map_err(|_| ControlError::StoreCorrupt)
    }
    fn u64(&mut self) -> Result<u64, ControlError> { Ok(u64::from_be_bytes(self.fixed()?)) }
    fn finish(self) -> Result<(), ControlError> { if self.0.is_empty() { Ok(()) } else { Err(ControlError::StoreCorrupt) } }
}

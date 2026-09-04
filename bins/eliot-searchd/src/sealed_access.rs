//! Append-only DPAPI access, scope, policy, and purge authority.
//!
//! One fence chain governs exactly one catalog-bound source revision. Every
//! record is immutable, transaction-backed, root/owner-bound, predecessor-hash
//! linked, and mutation-idempotent. Reads always validate the complete current
//! chain; a terminal `DENY` can never be reopened inside the same fence.

#![cfg_attr(not(windows), allow(dead_code))]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::sealed_access_codec::{
    ACCESS_FENCE_FORMAT_VERSION, AccessCodecError, AccessFenceRecord,
    AccessFenceState, validate_fence_id, validate_identifier, zero_digest,
};
use crate::sealed_digest::{DigestError, Sha256Digest, sha256};
use crate::sealed_owner_epoch::OwnerEpochGuard;
use crate::sealed_root_identity::{RootIdentityError, verify_owner_root};
use crate::sealed_store::{SealedStoreError, SensitiveBytes, open_sealed};
use crate::sealed_transaction::{
    SealedTransactionError, TransactionStatus, inspect_transaction,
};
use crate::sealed_transaction_guard::put_idempotent_verified;

/// Maximum generations in one access-fence chain.
pub const MAX_ACCESS_FENCE_GENERATIONS: usize = 1_000_000;
const SEALED_SUFFIX: &str = ".els-dpapi";
const SEALED_DIRECTORY: &str = "sealed-revisions";

/// Closed access authority failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedAccessError {
    /// The current platform cannot enumerate Windows sealed objects.
    UnsupportedPlatform,
    /// A fence has no generation.
    FenceNotFound,
    /// Current fence is terminally denied.
    AccessDenied,
    /// Source, revision, catalog, scope, or policy identity differs.
    AuthorityBindingMismatch,
    /// Mutation identity was reused with another exact payload.
    MutationConflict,
    /// Generation or access-generation continuity failed.
    GenerationConflict,
    /// Scope, policy, or purge revision regressed.
    RevisionRegression,
    /// A new record followed terminal `DENY`.
    DenyIsTerminal,
    /// Sealed access-fence object inventory is malformed or non-contiguous.
    ChainInvalid,
    /// Required transaction is not terminally committed.
    TransactionNotCommitted,
    /// Finite generation capacity was exhausted.
    CapacityExceeded,
    /// Filesystem inventory observation failed.
    IoFailure,
    /// Strict record codec failed.
    Codec(AccessCodecError),
    /// Windows CNG SHA-256 failed.
    Digest(DigestError),
    /// Native physical-root verification failed.
    RootIdentity(RootIdentityError),
    /// DPAPI sealed-object operation failed.
    SealedStore(SealedStoreError),
    /// Transaction inspection or reconciliation failed.
    Transaction(SealedTransactionError),
}

impl SealedAccessError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "SEALED_ACCESS_UNSUPPORTED_PLATFORM",
            Self::FenceNotFound => "SEALED_ACCESS_FENCE_NOT_FOUND",
            Self::AccessDenied => "SEALED_ACCESS_DENIED",
            Self::AuthorityBindingMismatch => "SEALED_ACCESS_AUTHORITY_BINDING_MISMATCH",
            Self::MutationConflict => "SEALED_ACCESS_MUTATION_CONFLICT",
            Self::GenerationConflict => "SEALED_ACCESS_GENERATION_CONFLICT",
            Self::RevisionRegression => "SEALED_ACCESS_REVISION_REGRESSION",
            Self::DenyIsTerminal => "SEALED_ACCESS_DENY_IS_TERMINAL",
            Self::ChainInvalid => "SEALED_ACCESS_CHAIN_INVALID",
            Self::TransactionNotCommitted => "SEALED_ACCESS_TRANSACTION_NOT_COMMITTED",
            Self::CapacityExceeded => "SEALED_ACCESS_CAPACITY_EXCEEDED",
            Self::IoFailure => "SEALED_ACCESS_IO_FAILURE",
            Self::Codec(error) => error.code(),
            Self::Digest(error) => error.code(),
            Self::RootIdentity(error) => error.code(),
            Self::SealedStore(error) => error.code(),
            Self::Transaction(error) => error.code(),
        }
    }
}

impl fmt::Display for SealedAccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedAccessError {}

impl From<AccessCodecError> for SealedAccessError {
    fn from(error: AccessCodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<DigestError> for SealedAccessError {
    fn from(error: DigestError) -> Self {
        Self::Digest(error)
    }
}

impl From<RootIdentityError> for SealedAccessError {
    fn from(error: RootIdentityError) -> Self {
        Self::RootIdentity(error)
    }
}

impl From<SealedStoreError> for SealedAccessError {
    fn from(error: SealedStoreError) -> Self {
        Self::SealedStore(error)
    }
}

impl From<SealedTransactionError> for SealedAccessError {
    fn from(error: SealedTransactionError) -> Self {
        Self::Transaction(error)
    }
}

/// Exact operator mutation request. Generation and access generation are
/// assigned by the validated chain, never by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessFenceMutation {
    /// Stable fence-chain identity.
    pub fence_id: String,
    /// Immutable idempotency identity.
    pub mutation_id: String,
    /// Stable source identity.
    pub source_id: String,
    /// Immutable source revision.
    pub source_revision_id: String,
    /// Exact catalog object.
    pub catalog_object_id: String,
    /// Stable scope identity.
    pub scope_id: String,
    /// Monotone scope revision.
    pub scope_revision: u64,
    /// Stable policy identity.
    pub policy_id: String,
    /// Monotone policy revision.
    pub policy_revision: u64,
    /// Monotone purge-ledger generation.
    pub purge_generation: u64,
    /// Desired terminal state.
    pub state: AccessFenceState,
}

impl AccessFenceMutation {
    fn validate(&self) -> Result<(), SealedAccessError> {
        validate_fence_id(&self.fence_id)?;
        for value in [
            &self.mutation_id,
            &self.source_id,
            &self.source_revision_id,
            &self.catalog_object_id,
            &self.scope_id,
            &self.policy_id,
        ] {
            validate_identifier(value)?;
        }
        if self.scope_revision == 0 || self.policy_revision == 0 {
            return Err(SealedAccessError::RevisionRegression);
        }
        Ok(())
    }
}

/// Access-fence append disposition.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AccessAppendDisposition {
    /// A new immutable generation was appended.
    Created,
    /// An existing exact mutation was replayed without appending.
    Replay,
}

impl AccessAppendDisposition {
    /// Stable wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Replay => "REPLAY",
        }
    }
}

/// Current exact fence snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessFenceSnapshot {
    /// Current record.
    pub record: AccessFenceRecord,
    /// SHA-256 of exact current record bytes.
    pub record_sha256: Sha256Digest,
    /// Current sealed object identity.
    pub object_id: String,
    /// Current transaction identity.
    pub transaction_id: String,
}

/// Append/replay receipt including the current head after the operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessFenceReceipt {
    /// Record created or originally associated with a replayed mutation.
    pub affected: AccessFenceSnapshot,
    /// Current chain head after the operation.
    pub current: AccessFenceSnapshot,
    /// Whether a generation was created or replayed.
    pub disposition: AccessAppendDisposition,
    /// Transaction and DPAPI readback were exact.
    pub readback_verified: bool,
}

/// Non-cloneable current read authority.
pub struct ActiveAccessFence {
    snapshot: AccessFenceSnapshot,
}

impl ActiveAccessFence {
    /// Current exact fence record.
    #[must_use]
    pub const fn record(&self) -> &AccessFenceRecord {
        &self.snapshot.record
    }

    /// Digest of current exact record bytes.
    #[must_use]
    pub const fn record_sha256(&self) -> Sha256Digest {
        self.snapshot.record_sha256
    }
}

impl fmt::Debug for ActiveAccessFence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveAccessFence")
            .field("fence_id", &self.snapshot.record.fence_id)
            .field("generation", &self.snapshot.record.generation)
            .field("access_generation", &self.snapshot.record.access_generation)
            .field("scope_revision", &self.snapshot.record.scope_revision)
            .field("policy_revision", &self.snapshot.record.policy_revision)
            .field("purge_generation", &self.snapshot.record.purge_generation)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoadedFence {
    snapshot: AccessFenceSnapshot,
}

/// Appends one exact `ALLOW` or terminal `DENY` mutation.
pub fn append_fence(
    data_root: &Path,
    owner: &OwnerEpochGuard,
    mutation: AccessFenceMutation,
) -> Result<AccessFenceReceipt, SealedAccessError> {
    verify_owner_root(data_root, owner)?;
    mutation.validate()?;
    let chain = load_chain(data_root, owner, &mutation.fence_id)?;

    if let Some(existing) = chain
        .iter()
        .find(|entry| entry.snapshot.record.mutation_id == mutation.mutation_id)
    {
        if !record_matches_mutation(&existing.snapshot.record, &mutation) {
            return Err(SealedAccessError::MutationConflict);
        }
        let current = chain
            .last()
            .ok_or(SealedAccessError::ChainInvalid)?
            .snapshot
            .clone();
        return Ok(AccessFenceReceipt {
            affected: existing.snapshot.clone(),
            current,
            disposition: AccessAppendDisposition::Replay,
            readback_verified: true,
        });
    }

    let (generation, previous_generation, previous_sha, access_generation) =
        if let Some(previous) = chain.last() {
            validate_successor_request(&previous.snapshot.record, &mutation)?;
            (
                previous
                    .snapshot
                    .record
                    .generation
                    .checked_add(1)
                    .ok_or(SealedAccessError::CapacityExceeded)?,
                previous.snapshot.record.generation,
                previous.snapshot.record_sha256,
                previous
                    .snapshot
                    .record
                    .access_generation
                    .checked_add(1)
                    .ok_or(SealedAccessError::CapacityExceeded)?,
            )
        } else {
            if mutation.state != AccessFenceState::Allow {
                return Err(SealedAccessError::AccessDenied);
            }
            (1, 0, zero_digest()?, 1)
        };
    if usize::try_from(generation).unwrap_or(usize::MAX)
        > MAX_ACCESS_FENCE_GENERATIONS
    {
        return Err(SealedAccessError::CapacityExceeded);
    }

    let record = AccessFenceRecord {
        format_version: ACCESS_FENCE_FORMAT_VERSION,
        fence_id: mutation.fence_id,
        mutation_id: mutation.mutation_id,
        generation,
        previous_generation,
        previous_record_sha256: previous_sha,
        source_id: mutation.source_id,
        source_revision_id: mutation.source_revision_id,
        catalog_object_id: mutation.catalog_object_id,
        scope_id: mutation.scope_id,
        scope_revision: mutation.scope_revision,
        policy_id: mutation.policy_id,
        policy_revision: mutation.policy_revision,
        access_generation,
        purge_generation: mutation.purge_generation,
        state: mutation.state,
        admitted_owner_epoch: owner.epoch(),
        owner_root_binding_sha256: owner.root_binding_sha256(),
    };
    let encoded = record.encode()?;
    let record_sha256 = sha256(encoded.as_bytes())?;
    let object_id = object_id(&record.fence_id, generation);
    let transaction_id = transaction_id(&record.fence_id, generation);
    let transaction = put_idempotent_verified(
        data_root,
        &transaction_id,
        &object_id,
        SensitiveBytes::new(encoded.as_bytes().to_vec())?,
    )?;
    if transaction.object_id != object_id
        || transaction.operation_id != transaction_id
        || transaction.plaintext_sha256 != record_sha256
        || transaction.plaintext_bytes
            != u64::try_from(encoded.len())
                .map_err(|_| SealedAccessError::ChainInvalid)?
    {
        return Err(SealedAccessError::ChainInvalid);
    }
    let readback = open_sealed(data_root, &object_id)?;
    if readback.expose() != encoded.as_bytes()
        || AccessFenceRecord::decode(readback.expose())? != record
    {
        return Err(SealedAccessError::ChainInvalid);
    }
    let affected = AccessFenceSnapshot {
        record,
        record_sha256,
        object_id,
        transaction_id,
    };
    Ok(AccessFenceReceipt {
        affected: affected.clone(),
        current: affected,
        disposition: AccessAppendDisposition::Created,
        readback_verified: true,
    })
}

/// Reads and validates the current fence head, including its complete chain.
pub fn current_fence(
    data_root: &Path,
    owner: &OwnerEpochGuard,
    fence_id: &str,
) -> Result<AccessFenceSnapshot, SealedAccessError> {
    verify_owner_root(data_root, owner)?;
    validate_fence_id(fence_id)?;
    load_chain(data_root, owner, fence_id)?
        .last()
        .map(|entry| entry.snapshot.clone())
        .ok_or(SealedAccessError::FenceNotFound)
}

/// Requires the exact current `ALLOW` fence for one catalog-bound revision.
pub fn require_active_fence(
    data_root: &Path,
    owner: &OwnerEpochGuard,
    fence_id: &str,
    source_id: &str,
    source_revision_id: &str,
    catalog_object_id: &str,
) -> Result<ActiveAccessFence, SealedAccessError> {
    for value in [source_id, source_revision_id, catalog_object_id] {
        validate_identifier(value)?;
    }
    let snapshot = current_fence(data_root, owner, fence_id)?;
    let record = &snapshot.record;
    if record.source_id != source_id
        || record.source_revision_id != source_revision_id
        || record.catalog_object_id != catalog_object_id
    {
        return Err(SealedAccessError::AuthorityBindingMismatch);
    }
    if record.state != AccessFenceState::Allow {
        return Err(SealedAccessError::AccessDenied);
    }
    if record.admitted_owner_epoch > owner.epoch()
        || record.owner_root_binding_sha256 != owner.root_binding_sha256()
    {
        return Err(SealedAccessError::AuthorityBindingMismatch);
    }
    Ok(ActiveAccessFence { snapshot })
}

fn load_chain(
    data_root: &Path,
    owner: &OwnerEpochGuard,
    fence_id: &str,
) -> Result<Vec<LoadedFence>, SealedAccessError> {
    let inventory = platform::discover(data_root, fence_id)?;
    if inventory.len() > MAX_ACCESS_FENCE_GENERATIONS {
        return Err(SealedAccessError::CapacityExceeded);
    }
    let mut chain = Vec::with_capacity(inventory.len());
    let mut mutation_ids = BTreeSet::new();

    for (index, (generation, object_id)) in inventory.into_iter().enumerate() {
        let expected_generation = u64::try_from(index)
            .map_err(|_| SealedAccessError::CapacityExceeded)?
            .checked_add(1)
            .ok_or(SealedAccessError::CapacityExceeded)?;
        if generation != expected_generation {
            return Err(SealedAccessError::ChainInvalid);
        }
        let plaintext = open_sealed(data_root, &object_id)?;
        let record = AccessFenceRecord::decode(plaintext.expose())?;
        if record.fence_id != fence_id
            || record.generation != generation
            || record.owner_root_binding_sha256 != owner.root_binding_sha256()
            || record.admitted_owner_epoch > owner.epoch()
            || !mutation_ids.insert(record.mutation_id.clone())
        {
            return Err(SealedAccessError::ChainInvalid);
        }
        let encoded = record.encode()?;
        if encoded.as_bytes() != plaintext.expose() {
            return Err(SealedAccessError::ChainInvalid);
        }
        let record_sha256 = sha256(plaintext.expose())?;
        if let Some(previous) = chain.last() {
            validate_successor_record(&previous.snapshot, &record)?;
        }
        let transaction_id = transaction_id(fence_id, generation);
        let transaction = put_idempotent_verified(
            data_root,
            &transaction_id,
            &object_id,
            SensitiveBytes::new(encoded.into_bytes())?,
        )?;
        if transaction.plaintext_sha256 != record_sha256
            || transaction.object_id != object_id
            || transaction.operation_id != transaction_id
        {
            return Err(SealedAccessError::ChainInvalid);
        }
        let observed = inspect_transaction(data_root, &transaction_id)?;
        if observed.status != TransactionStatus::Committed {
            return Err(SealedAccessError::TransactionNotCommitted);
        }
        chain.push(LoadedFence {
            snapshot: AccessFenceSnapshot {
                record,
                record_sha256,
                object_id,
                transaction_id,
            },
        });
    }
    Ok(chain)
}

fn validate_successor_record(
    previous: &AccessFenceSnapshot,
    current: &AccessFenceRecord,
) -> Result<(), SealedAccessError> {
    let previous_record = &previous.record;
    if previous_record.state == AccessFenceState::Deny {
        return Err(SealedAccessError::DenyIsTerminal);
    }
    if current.previous_generation != previous_record.generation
        || current.previous_record_sha256 != previous.record_sha256
        || current.generation != previous_record.generation.saturating_add(1)
        || current.access_generation
            != previous_record.access_generation.saturating_add(1)
    {
        return Err(SealedAccessError::GenerationConflict);
    }
    if !same_authority(previous_record, current) {
        return Err(SealedAccessError::AuthorityBindingMismatch);
    }
    if current.scope_revision < previous_record.scope_revision
        || current.policy_revision < previous_record.policy_revision
        || current.purge_generation < previous_record.purge_generation
    {
        return Err(SealedAccessError::RevisionRegression);
    }
    Ok(())
}

fn validate_successor_request(
    previous: &AccessFenceRecord,
    mutation: &AccessFenceMutation,
) -> Result<(), SealedAccessError> {
    if previous.state == AccessFenceState::Deny {
        return Err(SealedAccessError::DenyIsTerminal);
    }
    if previous.source_id != mutation.source_id
        || previous.source_revision_id != mutation.source_revision_id
        || previous.catalog_object_id != mutation.catalog_object_id
        || previous.scope_id != mutation.scope_id
        || previous.policy_id != mutation.policy_id
    {
        return Err(SealedAccessError::AuthorityBindingMismatch);
    }
    if mutation.scope_revision < previous.scope_revision
        || mutation.policy_revision < previous.policy_revision
        || mutation.purge_generation < previous.purge_generation
    {
        return Err(SealedAccessError::RevisionRegression);
    }
    Ok(())
}

fn same_authority(left: &AccessFenceRecord, right: &AccessFenceRecord) -> bool {
    left.fence_id == right.fence_id
        && left.source_id == right.source_id
        && left.source_revision_id == right.source_revision_id
        && left.catalog_object_id == right.catalog_object_id
        && left.scope_id == right.scope_id
        && left.policy_id == right.policy_id
        && left.owner_root_binding_sha256 == right.owner_root_binding_sha256
}

fn record_matches_mutation(
    record: &AccessFenceRecord,
    mutation: &AccessFenceMutation,
) -> bool {
    record.fence_id == mutation.fence_id
        && record.mutation_id == mutation.mutation_id
        && record.source_id == mutation.source_id
        && record.source_revision_id == mutation.source_revision_id
        && record.catalog_object_id == mutation.catalog_object_id
        && record.scope_id == mutation.scope_id
        && record.scope_revision == mutation.scope_revision
        && record.policy_id == mutation.policy_id
        && record.policy_revision == mutation.policy_revision
        && record.purge_generation == mutation.purge_generation
        && record.state == mutation.state
}

fn object_id(fence_id: &str, generation: u64) -> String {
    format!("access-fence-{fence_id}-{generation:020}")
}

fn transaction_id(fence_id: &str, generation: u64) -> String {
    format!("access-fence-op-{fence_id}-{generation:020}")
}

#[cfg(not(windows))]
mod platform {
    use super::SealedAccessError;
    use std::collections::BTreeMap;
    use std::path::Path;

    pub(super) fn discover(
        _data_root: &Path,
        _fence_id: &str,
    ) -> Result<BTreeMap<u64, String>, SealedAccessError> {
        Err(SealedAccessError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        MAX_ACCESS_FENCE_GENERATIONS, SEALED_DIRECTORY, SEALED_SUFFIX,
        SealedAccessError,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::windows::fs::MetadataExt;
    use std::path::Path;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

    pub(super) fn discover(
        data_root: &Path,
        fence_id: &str,
    ) -> Result<BTreeMap<u64, String>, SealedAccessError> {
        let directory = data_root.join(SEALED_DIRECTORY);
        if !directory.exists() {
            return Ok(BTreeMap::new());
        }
        let metadata = fs::symlink_metadata(&directory)
            .map_err(|_| SealedAccessError::IoFailure)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(SealedAccessError::ChainInvalid);
        }
        let prefix = format!("access-fence-{fence_id}-");
        let mut records = BTreeMap::new();
        for entry in fs::read_dir(&directory)
            .map_err(|_| SealedAccessError::IoFailure)?
        {
            let entry = entry.map_err(|_| SealedAccessError::IoFailure)?;
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if !file_name.starts_with(&prefix) {
                continue;
            }
            let entry_metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| SealedAccessError::IoFailure)?;
            if !entry_metadata.is_file()
                || entry_metadata.file_type().is_symlink()
                || entry_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(SealedAccessError::ChainInvalid);
            }
            let object_id = file_name
                .strip_suffix(SEALED_SUFFIX)
                .ok_or(SealedAccessError::ChainInvalid)?;
            let generation_text = object_id
                .strip_prefix(&prefix)
                .ok_or(SealedAccessError::ChainInvalid)?;
            if generation_text.len() != 20
                || !generation_text.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(SealedAccessError::ChainInvalid);
            }
            let generation = generation_text
                .parse::<u64>()
                .map_err(|_| SealedAccessError::ChainInvalid)?;
            if generation == 0
                || format!("{generation:020}") != generation_text
                || records.insert(generation, object_id.to_owned()).is_some()
                || records.len() > MAX_ACCESS_FENCE_GENERATIONS
            {
                return Err(SealedAccessError::ChainInvalid);
            }
        }
        Ok(records)
    }
}

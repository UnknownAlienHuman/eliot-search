//! Finite schema-5 journal operation modes, independent of indexed availability.

use super::operation::{Budget, Check, Point};
use super::{
    Boundary, ControlCallError, ControlCommitReceipt, ControlError, ControlKey, ControlMutation,
    JournalIdentity, JournalLimits, JournalReadSnapshot, MutationId, PersistentControlJournal,
    PublicationVisibilityState, RECORDS, ReadTransaction, decode_value, map_storage_error,
    map_table_error, operation_from_checked, request_fingerprint,
};
use crate::conditions::bind_conditions;
use crate::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlRecordClass, ControlRecordCondition,
    ControlValue, ControlWrite,
};
use search_contracts::{Blake3Digest32, OwnerEpoch};
use search_ports::{CancellationProbe, OperationContext};

pub(super) const KEY: &[u8] = b"eliot.control.operating-profile.v1";
const MAGIC: &[u8; 8] = b"ELJPRO01";

/// Closed technical operating mode; it grants no native, source or query authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalOperatingProfile {
    /// Source/control metadata and exact capability without an indexed route.
    Direct,
    /// Indexed protocols require their complete qualified visibility state.
    Indexed,
    /// Original-owner migration metadata only, unavailable to normal admission.
    MigrationInert,
}

/// Generation-zero profile input. Indexed initialization retains the actual supplied seed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalProfileInitialization {
    /// Initialize source/control operation without an index.
    Direct,
    /// Atomically initialize the profile and the supplied real indexed route.
    Indexed(PublicationVisibilityState),
    /// Initialize inert migration bookkeeping without normal registry admission.
    MigrationInert,
}

impl JournalProfileInitialization {
    const fn mode(self) -> JournalOperatingProfile {
        match self {
            Self::Direct => JournalOperatingProfile::Direct,
            Self::Indexed(_) => JournalOperatingProfile::Indexed,
            Self::MigrationInert => JournalOperatingProfile::MigrationInert,
        }
    }
}

struct Record {
    initial: JournalProfileInitialization,
    operation: MutationId,
    epoch: OwnerEpoch,
}

impl Record {
    fn value(&self, limits: JournalLimits) -> Result<ControlValue, ControlError> {
        let mut out = MAGIC.to_vec();
        out.push(match self.initial {
            JournalProfileInitialization::Direct => 1,
            JournalProfileInitialization::Indexed(_) => 2,
            JournalProfileInitialization::MigrationInert => 3,
        });
        out.extend_from_slice(&self.operation.0);
        out.extend_from_slice(&self.epoch.get().to_be_bytes());
        if let JournalProfileInitialization::Indexed(state) = self.initial {
            out.extend_from_slice(
                super::publication::visibility::codec::state(&state, limits)?.as_bytes(),
            );
        }
        ControlValue::new(ControlRecordClass::State, out, limits)
    }

    fn decode(value: &ControlValue, limits: JournalLimits) -> Result<Self, ControlError> {
        let bytes = value.as_bytes();
        if value.class() != ControlRecordClass::State || bytes.len() < 49 || &bytes[..8] != MAGIC {
            return Err(ControlError::StoreCorrupt);
        }
        let operation = MutationId(
            bytes[9..41]
                .try_into()
                .map_err(|_| ControlError::StoreCorrupt)?,
        );
        let epoch = OwnerEpoch::new(u64::from_be_bytes(
            bytes[41..49]
                .try_into()
                .map_err(|_| ControlError::StoreCorrupt)?,
        ))
        .map_err(|_| ControlError::StoreCorrupt)?;
        let initial = match bytes[8] {
            1 if bytes.len() == 49 => JournalProfileInitialization::Direct,
            3 if bytes.len() == 49 => JournalProfileInitialization::MigrationInert,
            2 => {
                let value =
                    ControlValue::new(ControlRecordClass::Revision, bytes[49..].to_vec(), limits)
                        .map_err(|_| ControlError::StoreCorrupt)?;
                JournalProfileInitialization::Indexed(
                    super::publication::visibility::codec::read_state(&value)?,
                )
            }
            _ => return Err(ControlError::StoreCorrupt),
        };
        Ok(Self {
            initial,
            operation,
            epoch,
        })
    }
}

fn command(
    initial: JournalProfileInitialization,
    identity: JournalIdentity,
    operation: MutationId,
    digest: Blake3Digest32,
    limits: JournalLimits,
) -> Result<ConditionalControlMutation, ControlError> {
    if identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
        return Err(ControlError::SchemaUnsupported);
    }
    let key = ControlKey::new(KEY.to_vec(), limits)?;
    let value = Record {
        initial,
        operation,
        epoch: identity.owner_epoch,
    }
    .value(limits)?;
    let mut writes = vec![ControlWrite {
        key: key.clone(),
        value,
    }];
    let conditions = vec![
        ControlRecordCondition::absent(key),
        ControlRecordCondition::absent(ControlKey::new(
            super::publication::visibility::STATE.to_vec(),
            limits,
        )?),
        ControlRecordCondition::absent(ControlKey::new(super::publication::KEY.to_vec(), limits)?),
    ];
    if let JournalProfileInitialization::Indexed(state) = initial {
        if state.visible_epoch.get() != 0
            || state.last_receipt.is_some()
            || state.guards.owner_epoch != identity.owner_epoch
        {
            return Err(ControlError::InvalidValue);
        }
        let key = ControlKey::new(super::publication::visibility::STATE.to_vec(), limits)?;
        writes.push(ControlWrite {
            key,
            value: super::publication::visibility::codec::state(&state, limits)?,
        });
    }
    // The exact existing transaction kernel charges writes + conditions.
    Ok(ConditionalControlMutation::new(
        ControlMutation::new(operation, digest, 0, writes, vec![]),
        conditions,
    ))
}

impl PersistentControlJournal {
    /// Atomically initializes the immutable schema-5 profile at generation zero.
    /// No default mode, serving authority or implicit transition is created.
    /// # Errors
    /// Existing data, conflicting identities, invalid visibility and bounds refuse;
    /// a possibly dispatched operation stays unknown until exact recovery.
    pub fn initialize_operating_profile<C: CancellationProbe>(
        &mut self,
        initial: JournalProfileInitialization,
        operation: MutationId,
        digest: Blake3Digest32,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, ControlCallError> {
        let budget = Budget::new(context);
        self.initialize_profile_checked(initial, operation, digest, Boundary::Normal, &budget)
            .map_err(|error| budget.failure(error, Some(operation)))
    }

    pub(in crate::persistent) fn initialize_profile_checked(
        &mut self,
        initial: JournalProfileInitialization,
        operation: MutationId,
        digest: Blake3Digest32,
        boundary: Boundary,
        check: &dyn Check,
    ) -> Result<ControlCommitReceipt, ControlError> {
        check.check(Point::Start)?;
        let command = command(initial, self.identity, operation, digest, self.limits)?;
        super::transaction::execute_profile(
            self,
            command.mutation(),
            command.conditions(),
            boundary,
            check,
        )
    }

    /// Resolves only this original initialization, without replaying a write.
    /// # Errors
    /// Different input cannot clear pending work; interrupted recovery remains unknown.
    pub fn recover_operating_profile_initialization<C: CancellationProbe>(
        &mut self,
        initial: JournalProfileInitialization,
        operation: MutationId,
        digest: Blake3Digest32,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, ControlCallError> {
        let budget = Budget::new(context);
        let result = (|| {
            budget.check(Point::Start)?;
            let command = command(initial, self.identity, operation, digest, self.limits)?;
            self.recover_transaction_with_conditions_checked(
                command.mutation(),
                command.conditions(),
                &budget,
            )
        })();
        result.map_err(|error| {
            budget
                .failure(
                    if budget.interrupted() {
                        ControlError::CommitOutcomeUnknown
                    } else {
                        error
                    },
                    Some(operation),
                )
                .for_recovery()
        })
    }

    /// Observes the actual immutable profile in one bounded read, without admission.
    /// # Errors
    /// Uninitialized, old-schema, pending, corrupt and interrupted state refuses.
    pub fn operating_profile<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<JournalOperatingProfile, ControlCallError> {
        let budget = Budget::new(context);
        let result = self
            .ensure_available()
            .and_then(|()| self.profile_checked(&budget));
        result.map_err(|error| budget.failure(error, None))
    }

    pub(super) fn profile_checked(
        &self,
        check: &dyn Check,
    ) -> Result<JournalOperatingProfile, ControlError> {
        check.check(Point::Start)?;
        let read = self
            .database
            .begin_read()
            .map_err(|_| ControlError::StoreUnavailable)?;
        self.header_from(&read)?;
        check.check(Point::ReadHeader)?;
        check.check(Point::ReadRecord)?;
        read_mode(&read, self.identity, self.limits)
    }

    pub(super) fn require_normal_profile(&self, check: &dyn Check) -> Result<(), ControlError> {
        if self.identity.schema_version == crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            normal(self.profile_checked(check)?)?;
        }
        Ok(())
    }
}

pub(super) fn normal(mode: JournalOperatingProfile) -> Result<(), ControlError> {
    if mode == JournalOperatingProfile::MigrationInert {
        Err(ControlError::MigrationUnverified)
    } else {
        Ok(())
    }
}

pub(super) fn read_mode(
    read: &ReadTransaction,
    identity: JournalIdentity,
    limits: JournalLimits,
) -> Result<JournalOperatingProfile, ControlError> {
    if identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
        return Err(ControlError::SchemaUnsupported);
    }
    let table = read.open_table(RECORDS).map_err(map_table_error)?;
    let value = table
        .get(KEY)
        .map_err(|e| map_storage_error(&e))?
        .ok_or(ControlError::SchemaMismatch)?;
    Ok(
        Record::decode(&decode_value(value.value(), limits)?, limits)?
            .initial
            .mode(),
    )
}

pub(super) fn require_indexed(journal: &PersistentControlJournal) -> Result<(), ControlError> {
    if journal.identity.schema_version == crate::REGISTRY_JOURNAL_SCHEMA_VERSION
        && journal.profile_checked(&super::Unscoped)? != JournalOperatingProfile::Indexed
    {
        return Err(ControlError::MigrationUnverified);
    }
    Ok(())
}

pub(super) fn snapshot_mode(
    snapshot: &JournalReadSnapshot,
    limits: JournalLimits,
) -> Result<JournalOperatingProfile, ControlError> {
    let key = ControlKey::new(KEY.to_vec(), limits)?;
    Record::decode(
        snapshot.get(&key).ok_or(ControlError::SchemaMismatch)?,
        limits,
    )
    .map(|r| r.initial.mode())
}

pub(super) fn mode_value(
    value: Option<&ControlValue>,
    limits: JournalLimits,
) -> Result<JournalOperatingProfile, ControlError> {
    Record::decode(value.ok_or(ControlError::SchemaMismatch)?, limits).map(|r| r.initial.mode())
}

pub(super) fn validate_snapshot(
    journal: &PersistentControlJournal,
    read: &ReadTransaction,
    snapshot: &JournalReadSnapshot,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let key = ControlKey::new(KEY.to_vec(), journal.limits)?;
    let Some(value) = snapshot.get(&key) else {
        return if snapshot.generation == 0 && snapshot.records.is_empty() {
            Ok(())
        } else {
            Err(ControlError::StoreCorrupt)
        };
    };
    let record = Record::decode(value, journal.limits)?;
    if record.epoch > snapshot.identity.owner_epoch {
        return Err(ControlError::StoreCorrupt);
    }
    let header = journal.header_from(read)?;
    check.check(Point::ReadOperation)?;
    let operation = operation_from_checked(read, record.operation, &header, journal.limits, check)?
        .ok_or(ControlError::StoreCorrupt)?;
    let original_identity = JournalIdentity {
        owner_epoch: record.epoch,
        ..snapshot.identity
    };
    let command = command(
        record.initial,
        original_identity,
        record.operation,
        operation.receipt.command_digest,
        journal.limits,
    )?;
    let fingerprint = bind_conditions(
        request_fingerprint(original_identity, command.mutation())?,
        command.conditions(),
        || check.check(Point::ReadRecord),
    )?;
    if operation.receipt.before_generation != 0
        || operation.receipt.after_generation != 1
        || operation.request_sha256 != fingerprint
    {
        return Err(ControlError::StoreCorrupt);
    }
    let mut keys = command
        .mutation()
        .writes()
        .iter()
        .map(|write| write.key.clone())
        .collect::<Vec<_>>();
    keys.sort();
    super::transaction::verify_replay(
        journal,
        read,
        &header,
        &operation.receipt,
        command.mutation(),
        &keys,
        check,
    )?;
    match record.initial.mode() {
        JournalOperatingProfile::Indexed => {
            super::publication::visibility::validate_snapshot(
                read,
                snapshot,
                journal.limits,
                check,
            )?;
        }
        JournalOperatingProfile::Direct | JournalOperatingProfile::MigrationInert => {
            if snapshot.records.iter().any(|(key, value)| {
                super::publication::port_reserved_key(key.as_bytes())
                    || (record.initial.mode() == JournalOperatingProfile::MigrationInert
                        && key.as_bytes() != KEY
                        && (super::registry::reserved(key.as_bytes())
                            || value.class() != ControlRecordClass::Migration))
            }) {
                return Err(ControlError::StoreCorrupt);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

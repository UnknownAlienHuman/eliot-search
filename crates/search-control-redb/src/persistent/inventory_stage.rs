//! Complete metadata-row adoption in the original, still inactive target.

use std::time::Instant;

use redb::{Durability, ReadTransaction, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::migration::{
    LegacyInventoryBinding, LegacyInventoryDecodedRow, LegacyInventoryDecoder,
    LegacyInventoryError, LegacyInventoryLimits, LegacyInventorySummary,
    MAX_SOURCE_IMPORT_ROW_BYTES, SourceImportRecordChain, SourceMappingComparison,
};
use crate::persistent::operation::{Check, Point};
use crate::{ControlError, MigrationBootstrapFailure, MigrationBootstrapWork as Work};

use super::{OriginalCheck, OwnerSourceMappedReceipt};

const ROWS: TableDefinition<u64, &[u8]> = TableDefinition::new("eliot.import.inventory.rows.v1");
const META: TableDefinition<&str, &[u8]> = TableDefinition::new("eliot.import.inventory.meta.v1");
const STAGE_KEY: &[u8] = b"migration/inventory-stage/v1/binding";
const BATCH: usize = 256;

/// Opaque continuation over the same already verified source-mapping owner.
/// Metadata evidence is not a complete semantic migration or cutover candidate.
pub struct OwnerInventoryStage {
    source: OwnerSourceMappedReceipt,
    decoder: Option<LegacyInventoryDecoder>,
    binding: LegacyInventoryBinding,
    marker: Vec<u8>,
    chain: SourceImportRecordChain,
    flushed: SourceImportRecordChain,
    pending: Vec<Vec<u8>>,
    latched: Option<ControlError>,
}

impl OwnerSourceMappedReceipt {
    /// Admits complete inventory adoption only from the original source receipt.
    /// No database reopen or caller summary can construct this continuation.
    /// # Errors
    /// Original refusal, changed proof or uncertain initialization retains the target.
    pub fn into_inventory_stage<F, E>(
        mut self,
        deadline: Instant,
        checkpoint: F,
    ) -> Result<OwnerInventoryStage, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            check.check(Point::Start)?;
            check.reserve(Work::Allocate(
                BATCH * std::mem::size_of::<Vec<u8>>()
                    + std::mem::size_of::<LegacyInventoryDecoder>()
                    + 16 * 1024,
            ))?;
            self.owner.bootstrap.journal.verify_checked(&check)?;
            check.reserve(Work::Process(16 * 1024))?;
            let original = self.owner.bootstrap.binding;
            let binding = LegacyInventoryBinding {
                owner: original.owner,
                target: original.target,
                namespace: original.legacy_namespace,
                snapshot: original.catalog_snapshot,
                registration: original.registration,
            };
            let profile = self.owner.bootstrap.completed_inventory.as_ref()
                .ok_or(ControlError::MigrationUnverified)?.profile;
            let decoder = LegacyInventoryDecoder::with_profile(binding, LegacyInventoryLimits::BASELINE, profile)
                .map_err(codec_error)?;
            let marker = marker(&self)?;
            self.owner.bootstrap.write_guarded(
                STAGE_KEY,
                marker.clone(),
                None,
                super::STAGE_KEY,
                self.owner.profile.clone(),
                &check,
            )?;
            let empty = SourceImportRecordChain::new();
            initialize(&self, &marker, &empty, &check)?;
            self.owner.bootstrap.journal.table_profile =
                crate::persistent::TableProfile::InventoryStage {
                    source: self.owner.profile.clone(),
                    inventory: marker.clone(),
                };
            self.owner.bootstrap.journal.verify_checked(&check)?;
            Ok(OwnerInventoryStage {
                source: self,
                decoder: Some(decoder),
                binding,
                marker,
                chain: empty.clone(),
                flushed: empty,
                pending: Vec::with_capacity(BATCH),
                latched: None,
            })
        })();
        result.map_err(|control| check.failure(control))
    }
}

impl OwnerInventoryStage {
    /// Decodes and adopts one bounded canonical inventory row, including terminals.
    /// # Errors
    /// Any refusal poisons this owner; committed prefixes cannot be blindly replayed.
    pub fn push<F, E>(
        &mut self,
        row: &[u8],
        deadline: Instant,
        checkpoint: F,
    ) -> Result<(), MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        if let Some(control) = self.latched {
            return Err(check.failure(control));
        }
        let result = (|| {
            admit_row(row, &check)?;
            self.decoder
                .as_mut()
                .ok_or(ControlError::MigrationUnverified)?
                .push(row)
                .map_err(codec_error)?;
            self.chain
                .push(row)
                .map_err(|_| ControlError::MigrationUnverified)?;
            self.pending.push(row.to_vec());
            if self.pending.len() == BATCH {
                self.flush(&check)?;
            }
            check.check(Point::ReadComplete)
        })();
        if let Err(control) = result {
            self.latched = Some(control);
        }
        result.map_err(|control| check.failure(control))
    }

    fn flush<F, E>(&mut self, check: &OriginalCheck<'_, F, E>) -> Result<(), ControlError>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        if self.pending.is_empty() {
            return check.check(Point::ReadComplete);
        }
        let bytes = self.chain.encoded_bytes() - self.flushed.encoded_bytes();
        check.reserve(Work::Write(
            usize::try_from(bytes).map_err(|_| ControlError::BudgetExceeded)?,
        ))?;
        let old = progress(&self.flushed);
        let next = progress(&self.chain);
        // Once a transaction may have begun, any refused observation stays unknown.
        let result = (|| {
            check.check(Point::BeforeWrite)?;
            let mut write = self
                .source
                .owner
                .bootstrap
                .journal
                .database
                .begin_write()
                .map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write
                    .open_table(META)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                require(&meta, "profile", &self.marker)?;
                require(&meta, "progress", &old)?;
                let mut rows = write
                    .open_table(ROWS)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                if rows.len().map_err(|_| ControlError::StoreUnavailable)? != self.flushed.rows() {
                    return Err(ControlError::MigrationUnverified);
                }
                for (offset, row) in self.pending.iter().enumerate() {
                    check.check(Point::ReadComplete)?;
                    let key = self.flushed.rows() + offset as u64;
                    if rows
                        .insert(key, row.as_slice())
                        .map_err(|_| ControlError::StoreUnavailable)?
                        .is_some()
                    {
                        return Err(ControlError::MigrationUnverified);
                    }
                }
                meta.insert("progress", next.as_slice())
                    .map_err(|_| ControlError::StoreUnavailable)?;
            }
            check.check(Point::BeforeCommit)?;
            write
                .commit()
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check.check(Point::AfterCommit)
        })();
        result.map_err(|_| ControlError::CommitOutcomeUnknown)?;
        self.flushed = self.chain.clone();
        self.pending.clear();
        Ok(())
    }

    fn seal_inventory<F, E>(
        &self,
        summary: &LegacyInventorySummary,
        check: &OriginalCheck<'_, F, E>,
    ) -> Result<Vec<u8>, ControlError>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        check.reserve(Work::Allocate(
            709 + std::mem::size_of::<LegacyInventoryDecoder>(),
        ))?;
        check.reserve(Work::Process(709))?;
        let sealed = seal(summary);
        check.reserve(Work::Write(sealed.len()))?;
        let commit = (|| {
            check.check(Point::BeforeWrite)?;
            let mut write = self
                .source
                .owner
                .bootstrap
                .journal
                .database
                .begin_write()
                .map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write
                    .open_table(META)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                require(&meta, "profile", &self.marker)?;
                require(&meta, "progress", &progress(&self.flushed))?;
                if meta.len().map_err(|_| ControlError::StoreUnavailable)? != 2
                    || meta
                        .insert("sealed", sealed.as_slice())
                        .map_err(|_| ControlError::StoreUnavailable)?
                        .is_some()
                {
                    return Err(ControlError::MigrationUnverified);
                }
            }
            check.check(Point::BeforeCommit)?;
            write
                .commit()
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check.check(Point::AfterCommit)
        })();
        commit.map_err(|_| ControlError::CommitOutcomeUnknown)?;
        Ok(sealed)
    }

    /// Seals only the fully decoded stream matching its original published artifact.
    /// # Errors
    /// Missing/footer/proof mismatch or unknown seal produces no readback capability.
    pub fn begin_readback<F, E>(
        mut self,
        deadline: Instant,
        checkpoint: F,
    ) -> Result<OwnerInventoryReadback, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            if let Some(control) = self.latched {
                return Err(control);
            }
            let summary = self
                .decoder
                .take()
                .ok_or(ControlError::MigrationUnverified)?
                .finish()
                .map_err(codec_error)?;
            let proof = self
                .source
                .owner
                .bootstrap
                .completed_inventory
                .as_ref()
                .ok_or(ControlError::MigrationUnverified)?;
            if self.chain.rows() != proof.rows
                || self.chain.encoded_bytes() != proof.bytes
                || self.chain.clone().finish() != proof.chain
                || summary.records != proof.rows
                || summary.encoded_bytes != proof.bytes
            {
                return Err(ControlError::MigrationUnverified);
            }
            let profile = proof.profile;
            self.flush(&check)?;
            let sealed = self.seal_inventory(&summary, &check)?;
            self.source.owner.bootstrap.journal.verify_checked(&check)?;
            check.reserve(Work::Read(MAX_SOURCE_IMPORT_ROW_BYTES))?;
            let transaction = self
                .source
                .owner
                .bootstrap
                .journal
                .database
                .begin_read()
                .map_err(|_| ControlError::StoreUnavailable)?;
            {
                let table = transaction
                    .open_table(META)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                if table.len().map_err(|_| ControlError::StoreUnavailable)? != 3 {
                    return Err(ControlError::MigrationUnverified);
                }
                require(&table, "profile", &self.marker)?;
                require(&table, "progress", &progress(&self.chain))?;
                require(&table, "sealed", &sealed)?;
                let rows = transaction
                    .open_table(ROWS)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                if rows.len().map_err(|_| ControlError::StoreUnavailable)? != summary.records {
                    return Err(ControlError::MigrationUnverified);
                }
            }
            let source = SourceMappingComparison::begin_inventory_in(
                &self.source.owner.bootstrap.journal.database,
                self.source.owner.binding,
                self.source.expected,
                &self.source.owner.profile,
                &self.marker,
                deadline,
                &|unit| check.reserve(unit),
            )?;
            let decoder =
                LegacyInventoryDecoder::with_profile(self.binding, LegacyInventoryLimits::BASELINE, profile)
                    .map_err(codec_error)?;
            check.check(Point::ReadComplete)?;
            Ok(OwnerInventoryReadback {
                transaction,
                source,
                owner: self,
                decoder,
                chain: SourceImportRecordChain::new(),
                summary,
                latched: None,
            })
        })();
        result.map_err(|control| check.failure(control))
    }
}

/// Pins read transactions before the owner so no vendor guard can outlive its database.
pub struct OwnerInventoryReadback {
    transaction: ReadTransaction,
    source: SourceMappingComparison,
    owner: OwnerInventoryStage,
    decoder: LegacyInventoryDecoder,
    chain: SourceImportRecordChain,
    summary: LegacyInventorySummary,
    latched: Option<ControlError>,
}

impl OwnerInventoryReadback {
    /// Compares every independently recompiled row and source-event index binding.
    /// # Errors
    /// Changed, missing, surplus or refused rows latch and cannot yield a receipt.
    pub fn compare<F, E>(
        &mut self,
        row: &[u8],
        deadline: Instant,
        checkpoint: F,
    ) -> Result<(), MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        if let Some(control) = self.latched {
            return Err(check.failure(control));
        }
        let result = (|| {
            admit_row(row, &check)?;
            let decoded = self.decoder.push(row).map_err(codec_error)?;
            check.reserve(Work::Read(MAX_SOURCE_IMPORT_ROW_BYTES))?;
            {
                let table = self
                    .transaction
                    .open_table(ROWS)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                let actual = table
                    .get(self.chain.rows())
                    .map_err(|_| ControlError::StoreUnavailable)?
                    .ok_or(ControlError::MigrationUnverified)?;
                if actual.value() != row {
                    return Err(ControlError::MigrationUnverified);
                }
            }
            if let LegacyInventoryDecodedRow::SourceEvent(source) = decoded {
                self.source
                    .compare(&source, deadline, &|unit| check.reserve(unit))?;
            }
            self.chain
                .push(row)
                .map_err(|_| ControlError::MigrationUnverified)?;
            check.check(Point::ReadComplete)
        })();
        if let Err(control) = result {
            self.latched = Some(control);
        }
        result.map_err(|control| check.failure(control))
    }

    /// Returns only fully read-back metadata, with unresolved semantic/recovery gaps.
    /// # Errors
    /// Partial comparison, changed accounting or original refusal returns no receipt.
    pub fn finish<F, E>(
        self,
        deadline: Instant,
        checkpoint: F,
    ) -> Result<OwnerInventoryMappedReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            if let Some(control) = self.latched {
                return Err(control);
            }
            let summary = self.decoder.finish().map_err(codec_error)?;
            if summary != self.summary
                || self.chain.rows() != self.owner.chain.rows()
                || self.chain.encoded_bytes() != self.owner.chain.encoded_bytes()
                || self.chain.finish() != self.owner.chain.clone().finish()
            {
                return Err(ControlError::MigrationUnverified);
            }
            let counts = self.source.finish(deadline, &|unit| check.reserve(unit))?;
            if counts != self.owner.source.expected {
                return Err(ControlError::MigrationUnverified);
            }
            self.owner
                .source
                .owner
                .bootstrap
                .journal
                .verify_checked(&check)?;
            check.check(Point::ReadComplete)?;
            drop(self.transaction);
            Ok(OwnerInventoryMappedReceipt {
                owner: self.owner,
                summary,
            })
        })();
        result.map_err(|control| check.failure(control))
    }
}

/// Opaque all-row metadata receipt. It cannot open a journal or authorize cutover.
pub struct OwnerInventoryMappedReceipt {
    owner: OwnerInventoryStage,
    summary: LegacyInventorySummary,
}
impl OwnerInventoryMappedReceipt {
    /// All five source dimensions and all twelve family accounting outcomes.
    #[must_use]
    pub fn render_json(&self) -> String {
        let counts = self.owner.source.expected;
        let families = crate::migration::LegacyInventoryFamily::ALL.into_iter().map(|family| {
            let observed = self.summary.counts[family as usize];
            format!("{{\"family\":\"{}\",\"counts\":[{},{},{},{},{},{},{}],\"denominator_complete\":{}}}",
                family.tag(), observed[0], observed[1], observed[2], observed[3], observed[4], observed[5], observed[6],
                self.summary.denominators[family as usize])
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"profile\":\"eliot.owner-inventory-stage.v1\",\"target\":\"{}\",\"inventory_rows\":{},\"inventory_bytes\":{},\"events\":{},\"sources\":{},\"occurrences\":{},\"retained_events\":{},\"retirements\":{},\"families\":[{}],\"gap\":\"complete_semantic_candidate_not_performed\",\"recovery\":\"unresolved\",\"cleanup\":\"deferred_to_347\",\"cutover\":\"none\"}}",
            self.owner.source.owner.bootstrap.target_name,
            self.summary.records,
            self.summary.encoded_bytes,
            counts.events,
            counts.sources,
            counts.occurrences,
            counts.retained_events,
            counts.retirements,
            families
        )
    }
}

fn admit_row<F, E>(row: &[u8], check: &OriginalCheck<'_, F, E>) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    if row.is_empty() || row.len() > MAX_SOURCE_IMPORT_ROW_BYTES {
        return Err(ControlError::BudgetExceeded);
    }
    check.reserve(Work::Allocate(row.len() * 4 + 4096))?;
    check.reserve(Work::Process(row.len() * 12 + 1024))
}

fn marker(source: &OwnerSourceMappedReceipt) -> Result<Vec<u8>, ControlError> {
    let proof = source
        .owner
        .bootstrap
        .completed_inventory
        .as_ref()
        .ok_or(ControlError::MigrationUnverified)?;
    let mut bytes = b"ELIINVM01".to_vec();
    bytes.extend_from_slice(&(source.owner.profile.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&source.owner.profile);
    bytes.extend_from_slice(&proof.chain);
    bytes.extend_from_slice(&proof.rows.to_be_bytes());
    bytes.extend_from_slice(&proof.bytes.to_be_bytes());
    for count in [
        source.expected.events,
        source.expected.sources,
        source.expected.occurrences,
        source.expected.retained_events,
        source.expected.retirements,
    ] {
        bytes.extend_from_slice(&count.to_be_bytes());
    }
    Ok(bytes)
}

fn progress(chain: &SourceImportRecordChain) -> [u8; 48] {
    let mut bytes = [0; 48];
    bytes[..8].copy_from_slice(&chain.rows().to_be_bytes());
    bytes[8..16].copy_from_slice(&chain.encoded_bytes().to_be_bytes());
    bytes[16..].copy_from_slice(&chain.clone().finish());
    bytes
}

fn seal(summary: &LegacyInventorySummary) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(709);
    bytes.extend_from_slice(b"ELIINVS01");
    bytes.extend_from_slice(&summary.records.to_be_bytes());
    bytes.extend_from_slice(&summary.encoded_bytes.to_be_bytes());
    for (counts, complete) in summary.counts.iter().zip(summary.denominators) {
        for count in counts {
            bytes.extend_from_slice(&count.to_be_bytes());
        }
        bytes.push(u8::from(complete));
    }
    bytes
}

fn initialize<F, E>(
    source: &OwnerSourceMappedReceipt,
    marker: &[u8],
    empty: &SourceImportRecordChain,
    check: &OriginalCheck<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    check.reserve(Work::Write(marker.len() + 48))?;
    let result = (|| {
        check.check(Point::BeforeWrite)?;
        let mut write = source
            .owner
            .bootstrap
            .journal
            .database
            .begin_write()
            .map_err(|_| ControlError::StoreUnavailable)?;
        write.set_durability(Durability::Immediate);
        {
            let mut meta = write
                .open_table(META)
                .map_err(|_| ControlError::StoreCorrupt)?;
            let rows = write
                .open_table(ROWS)
                .map_err(|_| ControlError::StoreCorrupt)?;
            if meta.len().map_err(|_| ControlError::StoreUnavailable)? != 0
                || rows.len().map_err(|_| ControlError::StoreUnavailable)? != 0
            {
                return Err(ControlError::MigrationUnverified);
            }
            meta.insert("profile", marker)
                .map_err(|_| ControlError::StoreUnavailable)?;
            meta.insert("progress", progress(empty).as_slice())
                .map_err(|_| ControlError::StoreUnavailable)?;
        }
        check.check(Point::BeforeCommit)?;
        write
            .commit()
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        check.check(Point::AfterCommit)
    })();
    result.map_err(|_| ControlError::CommitOutcomeUnknown)
}

fn require<T: ReadableTable<&'static str, &'static [u8]>>(
    table: &T,
    key: &str,
    expected: &[u8],
) -> Result<(), ControlError> {
    if table
        .get(key)
        .map_err(|_| ControlError::StoreUnavailable)?
        .is_none_or(|value| value.value() != expected)
    {
        return Err(ControlError::MigrationUnverified);
    }
    Ok(())
}

const fn codec_error(error: LegacyInventoryError) -> ControlError {
    match error {
        LegacyInventoryError::Limit => ControlError::BudgetExceeded,
        LegacyInventoryError::Accounting | LegacyInventoryError::Metadata => {
            ControlError::MigrationUnverified
        }
    }
}

#[cfg(test)]
#[path = "inventory_stage/tests.rs"]
mod tests;

//! Original-owner content continuation over the same inactive database.
//!
//! Only source-content metadata rows and fixed identity indexes are persisted.
//! Source bytes, locators, credentials and candidate authority remain outside
//! this owner.

use std::time::Instant;

use redb::{Durability, ReadTransaction, ReadableTable, ReadableTableMetadata, TableDefinition};
use search_contracts::Sha256Digest32;

use crate::migration::{
    ContentRevisionBinding, LegacyInventoryFamily, MAX_SOURCE_IMPORT_ROW_BYTES,
    SourceContentManifestDecodedRow, SourceContentManifestDecoder, SourceContentManifestEncoder,
    SourceContentManifestEncodingError, SourceContentManifestHeader, SourceContentManifestSummary,
    SourceContentObjectReadback, SourceImportPublishedRecordArtifact,
    SourceImportRecordArtifactPublication, SourceImportRecordChain, content_revision_binding,
    source_content_profile_digest,
};
use crate::persistent::TableProfile;
use crate::persistent::operation::{Check, Point};
use crate::{
    ControlError, ControlKey, MigrationBootstrapFailure, MigrationBootstrapIdentity,
    MigrationBootstrapWork as Work,
};

use super::super::super::{
    ATTEMPT_KEY, LIMITS, MigrationBootstrapPlatform, OPERATION_KEY, OriginalCheck, codec,
};
use super::OwnerInventoryMappedReceipt;

const CONTENT_PROFILE: &[u8] = b"eliot.owner-content-stage.v1\0control3+source4+inventory2+content3\0same-live-original-database\0source-content-v1\0no-serving-candidate-restart-cleanup\0batch=256;row=8192;occurrences<=2000000;owner-marker=4096\0";
const CONTENT_ATTEMPT_KEY: &[u8] = b"migration/content-stage/v1/content-attempt";
const BATCH: usize = 256;
const INDEX_VALUE_BYTES: usize = 80;
const SOURCE_META: TableDefinition<&str, &[u8]> =
    TableDefinition::new("eliot.import.source-map.meta.v1");
const INVENTORY_META: TableDefinition<&str, &[u8]> =
    TableDefinition::new("eliot.import.inventory.meta.v1");
const SOURCE_EVENTS: TableDefinition<u64, &[u8]> =
    TableDefinition::new("eliot.import.source-map.events.v1");
const SOURCE_REVISIONS: TableDefinition<&[u8], u64> =
    TableDefinition::new("eliot.import.source-map.revisions.v1");
const CONTENT_META: TableDefinition<&str, &[u8]> =
    TableDefinition::new("eliot.import.content.meta.v1");
const CONTENT_ROWS: TableDefinition<u64, &[u8]> =
    TableDefinition::new("eliot.import.content.rows.v1");
const CONTENT_BY_LEGACY: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("eliot.import.content.by-legacy.v1");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContentPhase {
    Prepared,
    Writing,
    Unknown,
}

struct PendingRow {
    ordinal: u64,
    bytes: Vec<u8>,
    index: Option<([u8; 64], [u8; INDEX_VALUE_BYTES])>,
}

/// Original-owner content staging, bound to the verified inventory receipt.
pub struct OwnerContentStage {
    owner: OwnerInventoryMappedReceipt,
    header: SourceContentManifestHeader,
    marker: Vec<u8>,
    phase: ContentPhase,
    published: Option<SourceImportPublishedRecordArtifact>,
    artifact_rows: Option<u64>,
    attempt: Option<Vec<u8>>,
    decoder: Option<SourceContentManifestDecoder>,
    chain: SourceImportRecordChain,
    flushed: SourceImportRecordChain,
    objects: u64,
    flushed_objects: u64,
    pending: Vec<PendingRow>,
    latched: Option<ControlError>,
}

impl OwnerInventoryMappedReceipt {
    /// Continues from the exact mapped inventory owner and retains its database.
    /// The caller's count is bounded by the verified occurrence denominator; it
    /// does not construct an owner or certify that any content was adopted.
    /// # Errors
    /// An absent/changed original proof, invalid count, or refused work fails closed.
    pub fn into_content_stage<F, E>(
        self,
        expected_objects: u64,
        deadline: Instant,
        checkpoint: F,
    ) -> Result<OwnerContentStage, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let result = (|| {
            check.check(Point::Start)?;
            check.reserve(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES + 12 * 1024))?;
            let counts = self.owner.source.expected;
            if expected_objects > counts.occurrences
                || !matches!(self.owner.source.owner.phase, super::super::Phase::Unknown)
            {
                return Err(ControlError::MigrationUnverified);
            }
            if self.owner.latched.is_some() || self.owner.decoder.is_some() {
                return Err(ControlError::MigrationUnverified);
            }
            let header = {
                let source = &self.owner.source;
                let bootstrap = &source.owner.bootstrap;
                let proof = bootstrap
                    .completed_inventory
                    .as_ref()
                    .ok_or(ControlError::MigrationUnverified)?;
                if source.artifact.chain() != source.owner.binding.plan_chain.as_bytes()
                    || self.owner.chain.rows() != proof.rows
                    || self.owner.chain.encoded_bytes() != proof.bytes
                    || self.owner.chain.clone().finish() != proof.chain
                {
                    return Err(ControlError::MigrationUnverified);
                }
                bootstrap.journal.verify_checked(&check)?;
                verify_owner_control_records(&self, None, &check)?;
                SourceContentManifestHeader {
                    target_namespace: bootstrap.binding.target,
                    legacy_namespace: bootstrap.binding.legacy_namespace,
                    catalog_snapshot: bootstrap.binding.catalog_snapshot,
                    source_plan: Sha256Digest32::from_bytes(*source.artifact.chain()),
                    expected_objects,
                }
            };
            check.reserve(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
            let mut encoder = SourceContentManifestEncoder::new(header).map_err(encoding_error)?;
            let header_row = encoder.header_row().map_err(encoding_error)?;
            let marker = content_marker(&self, header, &header_row)?;
            check.reserve(Work::Process(marker.len()))?;
            check.reserve(Work::Allocate(
                BATCH * std::mem::size_of::<PendingRow>() + BATCH * 64,
            ))?;

            Ok(OwnerContentStage {
                owner: self,
                header,
                marker,
                phase: ContentPhase::Prepared,
                published: None,
                artifact_rows: None,
                attempt: None,
                decoder: None,
                chain: SourceImportRecordChain::new(),
                flushed: SourceImportRecordChain::new(),
                objects: 0,
                flushed_objects: 0,
                pending: Vec::with_capacity(BATCH),
                latched: None,
            })
        })();
        result.map_err(|control| check.failure(control))
    }
}

impl OwnerContentStage {
    /// Records the bound attempt, publishes once, then initializes content tables.
    /// The decoder is admitted only after exact same-process publication readback.
    /// # Errors
    /// Any possibly dispatched or uncertain publication permanently refuses reuse.
    pub fn publish_content<P, F, E>(
        &mut self,
        publication: &mut SourceImportRecordArtifactPublication<P>,
        platform: &P,
        deadline: Instant,
        checkpoint: F,
    ) -> Result<(), MigrationBootstrapFailure<E>>
    where
        P: MigrationBootstrapPlatform,
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let mut dispatched = false;
        if self.phase != ContentPhase::Prepared {
            return Err(check.failure(ControlError::CommitOutcomeUnknown));
        }
        self.phase = ContentPhase::Unknown;
        let result = (|| {
            if self.published.is_some() || self.attempt.is_some() {
                return Err(ControlError::CommitOutcomeUnknown);
            }
            check.check(Point::Start)?;
            check.reserve(Work::Allocate(4096))?;
            let bootstrap = &self.owner.owner.source.owner.bootstrap;
            bootstrap.journal.verify_checked(&check)?;
            verify_owner_control_records(&self.owner, None, &check)?;
            let original_identity = publication
                .original_identity()
                .map_err(|_| ControlError::MigrationUnverified)?;
            let identity = platform.bootstrap_identity(original_identity);
            let marker_digest = (bootstrap.digest)(&self.marker);
            let pending = content_attempt(marker_digest, self.header, publication, identity, 0)?;
            let inventory_marker = self.owner.owner.marker.clone();
            self.owner.owner.source.owner.bootstrap.write_guarded(
                CONTENT_ATTEMPT_KEY,
                pending.clone(),
                None,
                super::STAGE_KEY,
                inventory_marker.clone(),
                &check,
            )?;
            check.check(Point::BeforeWrite)?;
            dispatched = true;
            let published = publication
                .publish_once_with_context(deadline, &|unit| check.reserve(unit))
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check.check(Point::AfterCommit)?;
            if published.reused()
                || published.name() != publication.final_name()
                || published.chain() != publication.chain()
                || published.encoded_bytes() != publication.encoded_bytes()
            {
                return Err(ControlError::CommitOutcomeUnknown);
            }
            let completed = content_attempt(marker_digest, self.header, publication, identity, 1)?;
            self.owner.owner.source.owner.bootstrap.write_guarded(
                CONTENT_ATTEMPT_KEY,
                completed.clone(),
                Some(pending),
                super::STAGE_KEY,
                inventory_marker,
                &check,
            )?;
            check.check(Point::MutationComplete)?;
            self.owner
                .owner
                .source
                .owner
                .bootstrap
                .journal
                .verify_checked(&check)?;
            verify_owner_control_records(&self.owner, Some(&completed), &check)?;

            initialize_content_tables(self, &published, &completed, &check)?;
            check.reserve(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
            let decoder = SourceContentManifestDecoder::new(self.header).map_err(encoding_error)?;
            self.published = Some(published);
            self.artifact_rows = Some(publication.rows());
            self.attempt = Some(completed);
            self.decoder = Some(decoder);
            self.phase = ContentPhase::Writing;
            check.check(Point::MutationComplete)
        })();
        result.map_err(|control| {
            check.failure(if dispatched {
                ControlError::CommitOutcomeUnknown
            } else {
                control
            })
        })
    }

    /// Decodes and batches one canonical source-content row into the same database.
    /// # Errors
    /// Malformed input or any uncertain write latches this owner permanently.
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
            if self.phase != ContentPhase::Writing {
                return Err(ControlError::CommitOutcomeUnknown);
            }
            super::admit_row(row, &check)?;
            let decoded = self
                .decoder
                .as_mut()
                .ok_or(ControlError::MigrationUnverified)?
                .push(row)
                .map_err(encoding_error)?;
            let ordinal = self.chain.rows();
            let index = match decoded {
                SourceContentManifestDecodedRow::Object(object) => {
                    let pair = legacy_pair(&object);
                    let value = index_value(ordinal, &object);
                    Some((pair, value))
                }
                SourceContentManifestDecodedRow::Header
                | SourceContentManifestDecodedRow::End(_) => None,
            };
            let objects = self
                .objects
                .checked_add(u64::from(index.is_some()))
                .ok_or(ControlError::BudgetExceeded)?;
            self.chain
                .push(row)
                .map_err(|_| ControlError::MigrationUnverified)?;
            self.pending.push(PendingRow {
                ordinal,
                bytes: row.to_vec(),
                index,
            });
            self.objects = objects;
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
        let row_bytes = self
            .chain
            .encoded_bytes()
            .checked_sub(self.flushed.encoded_bytes())
            .ok_or(ControlError::MigrationUnverified)?;
        let objects = self
            .objects
            .checked_sub(self.flushed_objects)
            .ok_or(ControlError::MigrationUnverified)?;
        let write_bytes = row_bytes
            .checked_add(
                objects
                    .checked_mul(64 + INDEX_VALUE_BYTES as u64)
                    .ok_or(ControlError::BudgetExceeded)?,
            )
            .and_then(|bytes| bytes.checked_add(56))
            .ok_or(ControlError::BudgetExceeded)?;
        check.reserve(Work::Write(
            usize::try_from(write_bytes).map_err(|_| ControlError::BudgetExceeded)?,
        ))?;
        let old = progress(&self.flushed, self.flushed_objects);
        let next = progress(&self.chain, self.objects);
        let marker = &self.marker;
        let attempt = self
            .attempt
            .as_deref()
            .ok_or(ControlError::MigrationUnverified)?;
        let database = &self.owner.owner.source.owner.bootstrap.journal.database;
        let result = (|| {
            check.check(Point::BeforeWrite)?;
            let mut write = database
                .begin_write()
                .map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write
                    .open_table(CONTENT_META)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                require(&meta, "profile", marker)?;
                require(&meta, "attempt", attempt)?;
                require(&meta, "progress", &old)?;
                let mut rows = write
                    .open_table(CONTENT_ROWS)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                let mut index = write
                    .open_table(CONTENT_BY_LEGACY)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                if rows.len().map_err(|_| ControlError::StoreUnavailable)? != self.flushed.rows()
                    || index.len().map_err(|_| ControlError::StoreUnavailable)?
                        != self.flushed_objects
                {
                    return Err(ControlError::MigrationUnverified);
                }
                for pending in &self.pending {
                    check.check(Point::ReadComplete)?;
                    if pending.ordinal != rows.len().map_err(|_| ControlError::StoreUnavailable)?
                        || rows
                            .insert(pending.ordinal, pending.bytes.as_slice())
                            .map_err(|_| ControlError::StoreUnavailable)?
                            .is_some()
                    {
                        return Err(ControlError::MigrationUnverified);
                    }
                    if let Some((legacy_pair, value)) = &pending.index
                        && index
                            .insert(legacy_pair.as_slice(), value.as_slice())
                            .map_err(|_| ControlError::StoreUnavailable)?
                            .is_some()
                    {
                        return Err(ControlError::MigrationUnverified);
                    }
                }
                if meta.len().map_err(|_| ControlError::StoreUnavailable)? != 3
                    || meta
                        .insert("progress", next.as_slice())
                        .map_err(|_| ControlError::StoreUnavailable)?
                        .is_none()
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
        result.map_err(|_| ControlError::CommitOutcomeUnknown)?;
        self.flushed = self.chain.clone();
        self.flushed_objects = self.objects;
        self.pending.clear();
        Ok(())
    }

    /// Seals the exact published stream and starts independent row readback.
    /// # Errors
    /// Incomplete, changed or unknown content state returns no readback owner.
    pub fn begin_readback<F, E>(
        mut self,
        deadline: Instant,
        checkpoint: F,
    ) -> Result<OwnerContentReadback, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        if self.phase != ContentPhase::Writing {
            return Err(check.failure(ControlError::CommitOutcomeUnknown));
        }
        self.phase = ContentPhase::Unknown;
        let result = (|| {
            if let Some(control) = self.latched {
                return Err(control);
            }
            if self.published.is_none() || self.attempt.is_none() {
                return Err(ControlError::CommitOutcomeUnknown);
            }
            let summary = self
                .decoder
                .take()
                .ok_or(ControlError::MigrationUnverified)?
                .finish()
                .map_err(encoding_error)?;
            let published = self
                .published
                .as_ref()
                .ok_or(ControlError::MigrationUnverified)?;
            if summary.objects != self.header.expected_objects
                || self.objects != summary.objects
                || Some(self.chain.rows()) != self.artifact_rows
                || self.chain.encoded_bytes() != published.encoded_bytes()
                || self.chain.clone().finish() != *published.chain()
            {
                return Err(ControlError::MigrationUnverified);
            }
            self.flush(&check)?;
            let sealed = seal(&summary, &self.chain);
            seal_content(&self, &summary, &sealed, &check)?;
            self.owner
                .owner
                .source
                .owner
                .bootstrap
                .journal
                .verify_checked(&check)?;
            verify_owner_control_records(&self.owner, self.attempt.as_deref(), &check)?;
            check.reserve(Work::Read(MAX_SOURCE_IMPORT_ROW_BYTES))?;
            let transaction = self
                .owner
                .owner
                .source
                .owner
                .bootstrap
                .journal
                .database
                .begin_read()
                .map_err(|_| ControlError::StoreUnavailable)?;
            verify_content_tables(&transaction, &self, &sealed)?;
            check.reserve(Work::Allocate(seen_bytes(summary.objects)?))?;
            let seen = vec![0; seen_bytes(summary.objects)?];
            check.reserve(Work::Allocate(MAX_SOURCE_IMPORT_ROW_BYTES))?;
            let decoder = SourceContentManifestDecoder::new(self.header).map_err(encoding_error)?;
            check.check(Point::ReadComplete)?;
            Ok(OwnerContentReadback {
                transaction,
                owner: self,
                decoder,
                chain: SourceImportRecordChain::new(),
                summary,
                seen,
                referenced_objects: 0,
                latched: None,
            })
        })();
        result.map_err(|control| check.failure(control))
    }

    #[cfg(test)]
    pub(super) fn table_count_for_test(&self) -> usize {
        self.owner
            .owner
            .source
            .owner
            .bootstrap
            .journal
            .database
            .begin_read()
            .expect("same owner read")
            .list_tables()
            .expect("table list")
            .count()
    }
}

/// Pins the original database snapshot before its retained owner.
pub struct OwnerContentReadback {
    transaction: ReadTransaction,
    owner: OwnerContentStage,
    decoder: SourceContentManifestDecoder,
    chain: SourceImportRecordChain,
    summary: SourceContentManifestSummary,
    seen: Vec<u8>,
    referenced_objects: u64,
    latched: Option<ControlError>,
}

impl OwnerContentReadback {
    /// Compares each independently regenerated V1 row and its legacy index entry.
    /// # Errors
    /// Any missing, changed, surplus or noncanonical row latches the reader.
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
            super::admit_row(row, &check)?;
            let ordinal = self.chain.rows();
            let decoded = self.decoder.push(row).map_err(encoding_error)?;
            check.reserve(Work::Read(MAX_SOURCE_IMPORT_ROW_BYTES))?;
            {
                let rows = self
                    .transaction
                    .open_table(CONTENT_ROWS)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                let stored = rows
                    .get(ordinal)
                    .map_err(|_| ControlError::StoreUnavailable)?
                    .ok_or(ControlError::MigrationUnverified)?;
                if stored.value() != row {
                    return Err(ControlError::MigrationUnverified);
                }
            }
            if let SourceContentManifestDecodedRow::Object(object) = decoded {
                let pair = legacy_pair(&object);
                let expected = index_value(ordinal, &object);
                let index = self
                    .transaction
                    .open_table(CONTENT_BY_LEGACY)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                let actual = index
                    .get(pair.as_slice())
                    .map_err(|_| ControlError::StoreUnavailable)?
                    .ok_or(ControlError::MigrationUnverified)?;
                if actual.value() != expected {
                    return Err(ControlError::MigrationUnverified);
                }
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

    /// Joins every canonical revision occurrence through its event row and exact legacy key.
    /// Missing historical content remains an explicit gap; orphan or mismatched
    /// content refuses the result.
    /// # Errors
    /// Partial row readback, an orphan object, or any identity/digest mismatch fails closed.
    pub fn finish<F, E>(
        self,
        deadline: Instant,
        checkpoint: F,
    ) -> Result<OwnerContentMappedReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::new(deadline, &checkpoint);
        let Self {
            transaction,
            owner,
            decoder,
            chain,
            summary,
            mut seen,
            mut referenced_objects,
            latched,
        } = self;
        let result = (|| {
            if let Some(control) = latched {
                return Err(control);
            }
            let decoded_summary = decoder.finish().map_err(encoding_error)?;
            let published = owner
                .published
                .as_ref()
                .ok_or(ControlError::MigrationUnverified)?;
            if decoded_summary != summary
                || summary.objects != owner.header.expected_objects
                || chain.rows() != owner.chain.rows()
                || chain.encoded_bytes() != owner.chain.encoded_bytes()
                || chain.clone().finish() != owner.chain.clone().finish()
                || Some(chain.rows()) != owner.artifact_rows
                || chain.encoded_bytes() != published.encoded_bytes()
                || chain.clone().finish() != *published.chain()
            {
                return Err(ControlError::MigrationUnverified);
            }
            let (joined_occurrences, missing_content_references, orphan_content_objects) =
                join_content_revisions(
                    &transaction,
                    &owner,
                    summary,
                    &mut seen,
                    &mut referenced_objects,
                    &check,
                )?;

            let content_chain = *published.chain();
            let content_rows = owner
                .artifact_rows
                .ok_or(ControlError::MigrationUnverified)?;
            let content_bytes = published.encoded_bytes();
            drop(transaction);
            owner
                .owner
                .owner
                .source
                .owner
                .bootstrap
                .journal
                .verify_checked(&check)?;
            verify_owner_control_records(&owner.owner, owner.attempt.as_deref(), &check)?;
            check.check(Point::ReadComplete)?;
            Ok(OwnerContentMappedReceipt {
                owner,
                summary,
                chain: content_chain,
                rows: content_rows,
                encoded_bytes: content_bytes,
                joined_occurrences,
                missing_content_references,
                orphan_content_objects,
            })
        })();
        result.map_err(|control| check.failure(control))
    }
}

fn join_content_revisions<F, E>(
    transaction: &ReadTransaction,
    owner: &OwnerContentStage,
    summary: SourceContentManifestSummary,
    seen: &mut [u8],
    referenced_objects: &mut u64,
    check: &OriginalCheck<'_, F, E>,
) -> Result<(u64, u64, u64), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    check.reserve(Work::Read(512))?;
    let source_meta = transaction
        .open_table(SOURCE_META)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let inventory_meta = transaction
        .open_table(INVENTORY_META)
        .map_err(|_| ControlError::StoreCorrupt)?;
    require(
        &source_meta,
        "owner_source_stage.v1",
        &owner.owner.owner.source.owner.profile,
    )?;
    require(&inventory_meta, "profile", &owner.owner.owner.marker)?;
    let events = transaction
        .open_table(SOURCE_EVENTS)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let revisions = transaction
        .open_table(SOURCE_REVISIONS)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let by_legacy = transaction
        .open_table(CONTENT_BY_LEGACY)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let expected = owner.owner.owner.source.expected;
    if events.len().map_err(|_| ControlError::StoreUnavailable)? != expected.events
        || revisions
            .len()
            .map_err(|_| ControlError::StoreUnavailable)?
            != expected.occurrences
        || by_legacy
            .len()
            .map_err(|_| ControlError::StoreUnavailable)?
            != summary.objects
    {
        return Err(ControlError::MigrationUnverified);
    }

    let mut joined_occurrences = 0_u64;
    let mut missing_content_references = 0_u64;
    let mut traversed = 0_u64;
    for entry in revisions
        .iter()
        .map_err(|_| ControlError::StoreUnavailable)?
    {
        check.reserve(Work::Read(371 + INDEX_VALUE_BYTES))?;
        check.reserve(Work::Process(371 + 256))?;
        let (revision_key, sequence) = entry.map_err(|_| ControlError::StoreUnavailable)?;
        let sequence = sequence.value();
        let event = events
            .get(sequence)
            .map_err(|_| ControlError::StoreUnavailable)?
            .ok_or(ControlError::MigrationUnverified)?;
        let binding = content_revision_binding(revision_key.value(), sequence, event.value())?;
        let source_content = by_legacy
            .get(binding.legacy_pair.as_slice())
            .map_err(|_| ControlError::StoreUnavailable)?;
        traversed = traversed
            .checked_add(1)
            .ok_or(ControlError::BudgetExceeded)?;
        if let Some(value) = source_content {
            validate_join_value(value.value(), &binding, summary.objects)?;
            let object_ordinal = u64::from_be_bytes(
                value.value()[..8]
                    .try_into()
                    .map_err(|_| ControlError::StoreCorrupt)?,
            );
            if mark_seen(seen, object_ordinal)? {
                *referenced_objects = referenced_objects
                    .checked_add(1)
                    .ok_or(ControlError::BudgetExceeded)?;
            }
            joined_occurrences = joined_occurrences
                .checked_add(1)
                .ok_or(ControlError::BudgetExceeded)?;
        } else {
            missing_content_references = missing_content_references
                .checked_add(1)
                .ok_or(ControlError::BudgetExceeded)?;
        }
        check.check(Point::ReadComplete)?;
    }
    let orphan_content_objects = validate_join_accounting(
        joined_occurrences,
        missing_content_references,
        *referenced_objects,
        expected.occurrences,
        summary.objects,
        traversed,
    )?;
    Ok((
        joined_occurrences,
        missing_content_references,
        orphan_content_objects,
    ))
}

/// Opaque partial content proof. It does not complete semantic adoption or cutover.
pub struct OwnerContentMappedReceipt {
    owner: OwnerContentStage,
    summary: SourceContentManifestSummary,
    chain: [u8; 32],
    rows: u64,
    encoded_bytes: u64,
    joined_occurrences: u64,
    missing_content_references: u64,
    orphan_content_objects: u64,
}

impl OwnerContentMappedReceipt {
    /// Content-minimized source, inventory and join accounting with all gaps explicit.
    #[must_use]
    pub fn render_json(&self) -> String {
        let inventory_receipt = &self.owner.owner;
        let inventory = &inventory_receipt.owner;
        let counts = inventory.source.expected;
        let families = LegacyInventoryFamily::ALL.into_iter().map(|family| {
            let observed = inventory_receipt.summary.counts[family as usize];
            format!(
                "{{\"family\":\"{}\",\"counts\":[{},{},{},{},{},{},{}],\"denominator_complete\":{}}}",
                family.tag(), observed[0], observed[1], observed[2], observed[3],
                observed[4], observed[5], observed[6],
                inventory_receipt.summary.denominators[family as usize]
            )
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"profile\":\"eliot.owner-content-stage.v1\",\"target\":\"{}\",\"inventory_rows\":{},\"inventory_bytes\":{},\"content_chain_sha256\":\"{}\",\"content_rows\":{},\"content_bytes\":{},\"content_objects\":{},\"source_bytes\":{},\"events\":{},\"sources\":{},\"occurrences\":{},\"retained_events\":{},\"retirements\":{},\"joined_occurrences\":{},\"missing_content_references\":{},\"orphan_content_objects\":{},\"families\":[{}],\"gap\":\"complete_semantic_candidate_not_performed\",\"recovery\":\"unresolved\",\"cleanup\":\"deferred_to_347\",\"cutover\":\"none\"}}",
            inventory.source.owner.bootstrap.target_name,
            inventory_receipt.summary.records,
            inventory_receipt.summary.encoded_bytes,
            codec::hex(&self.chain),
            self.rows,
            self.encoded_bytes,
            self.summary.objects,
            self.summary.source_bytes,
            counts.events,
            counts.sources,
            counts.occurrences,
            counts.retained_events,
            counts.retirements,
            self.joined_occurrences,
            self.missing_content_references,
            self.orphan_content_objects,
            families,
        )
    }
}

fn initialize_content_tables<F, E>(
    stage: &mut OwnerContentStage,
    published: &SourceImportPublishedRecordArtifact,
    attempt: &[u8],
    check: &OriginalCheck<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    let bootstrap = &mut stage.owner.owner.source.owner.bootstrap;
    check.reserve(Work::Write(stage.marker.len() + attempt.len() + 56))?;
    bootstrap.journal.verify_checked(check)?;
    let result = (|| {
        check.check(Point::BeforeWrite)?;
        let mut write = bootstrap
            .journal
            .database
            .begin_write()
            .map_err(|_| ControlError::StoreUnavailable)?;
        write.set_durability(Durability::Immediate);
        {
            let mut meta = write
                .open_table(CONTENT_META)
                .map_err(|_| ControlError::StoreCorrupt)?;
            let rows = write
                .open_table(CONTENT_ROWS)
                .map_err(|_| ControlError::StoreCorrupt)?;
            let index = write
                .open_table(CONTENT_BY_LEGACY)
                .map_err(|_| ControlError::StoreCorrupt)?;
            if meta.len().map_err(|_| ControlError::StoreUnavailable)? != 0
                || rows.len().map_err(|_| ControlError::StoreUnavailable)? != 0
                || index.len().map_err(|_| ControlError::StoreUnavailable)? != 0
            {
                return Err(ControlError::MigrationUnverified);
            }
            meta.insert("profile", stage.marker.as_slice())
                .map_err(|_| ControlError::StoreUnavailable)?;
            meta.insert("progress", progress(&stage.chain, 0).as_slice())
                .map_err(|_| ControlError::StoreUnavailable)?;
            meta.insert("attempt", attempt)
                .map_err(|_| ControlError::StoreUnavailable)?;
        }
        check.check(Point::BeforeCommit)?;
        write
            .commit()
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        check.check(Point::AfterCommit)
    })();
    result.map_err(|_| ControlError::CommitOutcomeUnknown)?;
    bootstrap.journal.table_profile = TableProfile::ContentStage {
        source: stage.owner.owner.source.owner.profile.clone(),
        inventory: stage.owner.owner.marker.clone(),
        content: stage.marker.clone(),
    };
    bootstrap.journal.verify_checked(check)?;
    if published.reused() {
        return Err(ControlError::CommitOutcomeUnknown);
    }
    Ok(())
}

fn seal_content<F, E>(
    stage: &OwnerContentStage,
    summary: &SourceContentManifestSummary,
    sealed: &[u8],
    check: &OriginalCheck<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    if stage.objects != summary.objects
        || summary.objects != stage.header.expected_objects
        || stage.flushed_objects != summary.objects
        || stage.flushed.rows() != stage.chain.rows()
        || stage.flushed.encoded_bytes() != stage.chain.encoded_bytes()
        || stage.flushed.clone().finish() != stage.chain.clone().finish()
    {
        return Err(ControlError::MigrationUnverified);
    }
    let attempt = stage
        .attempt
        .as_deref()
        .ok_or(ControlError::MigrationUnverified)?;
    let result = (|| {
        check.reserve(Work::Allocate(sealed.len()))?;
        check.reserve(Work::Process(sealed.len()))?;
        check.reserve(Work::Write(sealed.len()))?;
        check.check(Point::BeforeWrite)?;
        let mut write = stage
            .owner
            .owner
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
                .open_table(CONTENT_META)
                .map_err(|_| ControlError::StoreCorrupt)?;
            require(&meta, "profile", &stage.marker)?;
            require(&meta, "attempt", attempt)?;
            require(
                &meta,
                "progress",
                &progress(&stage.flushed, stage.flushed_objects),
            )?;
            if meta.len().map_err(|_| ControlError::StoreUnavailable)? != 3
                || meta
                    .insert("sealed", sealed)
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
    result.map_err(|_| ControlError::CommitOutcomeUnknown)
}

fn verify_content_tables(
    transaction: &ReadTransaction,
    stage: &OwnerContentStage,
    sealed: &[u8],
) -> Result<(), ControlError> {
    let meta = transaction
        .open_table(CONTENT_META)
        .map_err(|_| ControlError::StoreCorrupt)?;
    if meta.len().map_err(|_| ControlError::StoreUnavailable)? != 4 {
        return Err(ControlError::MigrationUnverified);
    }
    require(&meta, "profile", &stage.marker)?;
    require(
        &meta,
        "attempt",
        stage
            .attempt
            .as_deref()
            .ok_or(ControlError::MigrationUnverified)?,
    )?;
    require(&meta, "progress", &progress(&stage.chain, stage.objects))?;
    require(&meta, "sealed", sealed)?;
    let rows = transaction
        .open_table(CONTENT_ROWS)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let index = transaction
        .open_table(CONTENT_BY_LEGACY)
        .map_err(|_| ControlError::StoreCorrupt)?;
    if rows.len().map_err(|_| ControlError::StoreUnavailable)? != stage.chain.rows()
        || index.len().map_err(|_| ControlError::StoreUnavailable)? != stage.objects
    {
        return Err(ControlError::MigrationUnverified);
    }
    Ok(())
}

fn verify_owner_control_records<F, E>(
    receipt: &OwnerInventoryMappedReceipt,
    content_attempt: Option<&[u8]>,
    check: &OriginalCheck<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    check.reserve(Work::Read(4 * 4096))?;
    check.reserve(Work::Process(4 * 4096))?;
    let source = &receipt.owner.source;
    let bootstrap = &source.owner.bootstrap;
    let proof = bootstrap
        .completed_inventory
        .as_ref()
        .ok_or(ControlError::MigrationUnverified)?;
    let snapshot = bootstrap.journal.read_snapshot_checked(check)?;
    let original_key = ControlKey::new(OPERATION_KEY.to_vec(), LIMITS)?;
    let original = snapshot
        .get(&original_key)
        .ok_or(ControlError::MigrationUnverified)?;
    let inventory_attempt_key = ControlKey::new(ATTEMPT_KEY.to_vec(), LIMITS)?;
    let inventory_attempt = snapshot
        .get(&inventory_attempt_key)
        .ok_or(ControlError::MigrationUnverified)?;
    let source_stage_key = ControlKey::new(super::super::STAGE_KEY.to_vec(), LIMITS)?;
    let source_stage = snapshot
        .get(&source_stage_key)
        .ok_or(ControlError::MigrationUnverified)?;
    let source_attempt_key = ControlKey::new(super::super::MAP_ATTEMPT_KEY.to_vec(), LIMITS)?;
    let source_attempt = snapshot
        .get(&source_attempt_key)
        .ok_or(ControlError::MigrationUnverified)?;
    let inventory_stage_key = ControlKey::new(super::STAGE_KEY.to_vec(), LIMITS)?;
    let inventory_stage = snapshot
        .get(&inventory_stage_key)
        .ok_or(ControlError::MigrationUnverified)?;
    if original.as_bytes() != bootstrap.original.as_slice()
        || inventory_attempt.as_bytes() != proof.attempt
        || source_stage.as_bytes() != source.owner.profile
        || source_attempt.as_bytes().is_empty()
        || inventory_stage.as_bytes() != receipt.owner.marker
    {
        return Err(ControlError::IdentityMismatch);
    }

    let content_attempt_key = ControlKey::new(CONTENT_ATTEMPT_KEY.to_vec(), LIMITS)?;
    match (content_attempt, snapshot.get(&content_attempt_key)) {
        (Some(expected), Some(actual)) if actual.as_bytes() == expected => {}
        (None, None) => {}
        _ => return Err(ControlError::IdentityMismatch),
    }
    Ok(())
}

fn content_marker(
    receipt: &OwnerInventoryMappedReceipt,
    header: SourceContentManifestHeader,
    header_row: &[u8],
) -> Result<Vec<u8>, ControlError> {
    let inventory = &receipt.owner;
    let source = &inventory.source;
    let proof = source
        .owner
        .bootstrap
        .completed_inventory
        .as_ref()
        .ok_or(ControlError::MigrationUnverified)?;
    let mut out = Vec::with_capacity(3072);
    out.extend_from_slice(CONTENT_PROFILE);
    append_blob(&mut out, &source.owner.profile)?;
    append_blob(&mut out, &inventory.marker)?;
    append_blob(&mut out, &proof.attempt)?;
    append_blob(&mut out, source.artifact.name().as_bytes())?;
    out.extend_from_slice(source.artifact.chain());
    out.extend_from_slice(&source.expected.events.to_be_bytes());
    out.extend_from_slice(&source.artifact.encoded_bytes().to_be_bytes());
    out.extend_from_slice(proof.original.as_bytes());
    out.extend_from_slice(&proof.chain);
    out.extend_from_slice(&proof.rows.to_be_bytes());
    out.extend_from_slice(&proof.bytes.to_be_bytes());
    out.extend_from_slice(header.target_namespace.as_bytes());
    out.extend_from_slice(header.legacy_namespace.as_bytes());
    out.extend_from_slice(header.catalog_snapshot.as_bytes());
    out.extend_from_slice(header.source_plan.as_bytes());
    out.extend_from_slice(source_content_profile_digest().as_bytes());
    out.extend_from_slice(&header.expected_objects.to_be_bytes());
    append_blob(&mut out, header_row)?;
    for count in [
        source.expected.events,
        source.expected.sources,
        source.expected.occurrences,
        source.expected.retained_events,
        source.expected.retirements,
        receipt.summary.records,
        receipt.summary.encoded_bytes,
    ] {
        out.extend_from_slice(&count.to_be_bytes());
    }
    for (counts, complete) in receipt
        .summary
        .counts
        .iter()
        .zip(receipt.summary.denominators)
    {
        for count in counts {
            out.extend_from_slice(&count.to_be_bytes());
        }
        out.push(u8::from(complete));
    }
    if out.len() > 4096 {
        return Err(ControlError::BudgetExceeded);
    }
    Ok(out)
}

fn content_attempt<P: MigrationBootstrapPlatform>(
    marker_digest: search_contracts::Blake3Digest32,
    header: SourceContentManifestHeader,
    publication: &SourceImportRecordArtifactPublication<P>,
    identity: MigrationBootstrapIdentity,
    phase: u8,
) -> Result<Vec<u8>, ControlError> {
    let stage_name = publication
        .staging_name()
        .to_str()
        .ok_or(ControlError::MigrationUnverified)?;
    let final_name = publication.final_name();
    codec::local_name(stage_name)?;
    codec::local_name(final_name)?;
    let expected_name = format!("{}.source-content.v1", codec::hex(publication.chain()));
    let expected_rows = header
        .expected_objects
        .checked_add(2)
        .ok_or(ControlError::BudgetExceeded)?;
    if phase > 1
        || stage_name == final_name
        || final_name != expected_name
        || publication.rows() != expected_rows
        || publication.encoded_bytes() == 0
        || identity.file == 0
        || (identity.profile == crate::MigrationBootstrapIdentityProfile::WindowsFileIndex64
            && identity.volume > u64::from(u32::MAX))
    {
        return Err(ControlError::MigrationUnverified);
    }
    let capacity = stage_name
        .len()
        .checked_add(final_name.len())
        .and_then(|len| len.checked_add(256))
        .ok_or(ControlError::BudgetExceeded)?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(b"eliot.owner-content-attempt.v1\0");
    out.extend_from_slice(&marker_digest.as_bytes()[..]);
    out.push(phase);
    out.push(identity.profile as u8);
    out.extend_from_slice(&identity.volume.to_le_bytes());
    out.extend_from_slice(&identity.file.to_le_bytes());
    for name in [stage_name, final_name] {
        let length = u16::try_from(name.len()).map_err(|_| ControlError::BudgetExceeded)?;
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    out.extend_from_slice(publication.chain());
    out.extend_from_slice(&publication.rows().to_le_bytes());
    out.extend_from_slice(&publication.encoded_bytes().to_le_bytes());
    Ok(out)
}

fn progress(chain: &SourceImportRecordChain, objects: u64) -> [u8; 56] {
    let mut bytes = [0; 56];
    bytes[..8].copy_from_slice(&chain.rows().to_be_bytes());
    bytes[8..16].copy_from_slice(&objects.to_be_bytes());
    bytes[16..24].copy_from_slice(&chain.encoded_bytes().to_be_bytes());
    bytes[24..].copy_from_slice(&chain.clone().finish());
    bytes
}

fn seal(summary: &SourceContentManifestSummary, chain: &SourceImportRecordChain) -> [u8; 72] {
    let mut bytes = [0; 72];
    bytes[..8].copy_from_slice(b"ELSCSEAL");
    bytes[8..16].copy_from_slice(&summary.objects.to_be_bytes());
    bytes[16..24].copy_from_slice(&summary.source_bytes.to_be_bytes());
    bytes[24..32].copy_from_slice(&chain.rows().to_be_bytes());
    bytes[32..40].copy_from_slice(&chain.encoded_bytes().to_be_bytes());
    bytes[40..].copy_from_slice(&chain.clone().finish());
    bytes
}

fn legacy_pair(object: &SourceContentObjectReadback) -> [u8; 64] {
    let mut pair = [0; 64];
    pair[..32].copy_from_slice(object.legacy_source_id.as_bytes());
    pair[32..].copy_from_slice(object.legacy_revision_id.as_bytes());
    pair
}

fn index_value(ordinal: u64, object: &SourceContentObjectReadback) -> [u8; INDEX_VALUE_BYTES] {
    let mut value = [0; INDEX_VALUE_BYTES];
    value[..8].copy_from_slice(&ordinal.to_be_bytes());
    value[8..40].copy_from_slice(object.content_sha256.as_bytes());
    value[40..48].copy_from_slice(&object.byte_length.to_be_bytes());
    value[48..].copy_from_slice(object.content_blake3.as_bytes());
    value
}

fn validate_join_value(
    value: &[u8],
    binding: &ContentRevisionBinding,
    objects: u64,
) -> Result<(), ControlError> {
    if value.len() != INDEX_VALUE_BYTES {
        return Err(ControlError::StoreCorrupt);
    }
    let ordinal = u64::from_be_bytes(
        value[..8]
            .try_into()
            .map_err(|_| ControlError::StoreCorrupt)?,
    );
    let length = u64::from_be_bytes(
        value[40..48]
            .try_into()
            .map_err(|_| ControlError::StoreCorrupt)?,
    );
    if ordinal == 0
        || ordinal > objects
        || value[8..40] != binding.content_sha256
        || length != binding.source_bytes
    {
        return Err(ControlError::MigrationUnverified);
    }
    Ok(())
}

fn validate_join_accounting(
    joined_occurrences: u64,
    missing_content_references: u64,
    referenced_objects: u64,
    expected_occurrences: u64,
    content_objects: u64,
    traversed: u64,
) -> Result<u64, ControlError> {
    if traversed != expected_occurrences
        || joined_occurrences.checked_add(missing_content_references) != Some(expected_occurrences)
    {
        return Err(ControlError::MigrationUnverified);
    }
    let orphan_content_objects = content_objects
        .checked_sub(referenced_objects)
        .ok_or(ControlError::MigrationUnverified)?;
    if orphan_content_objects != 0 {
        return Err(ControlError::MigrationUnverified);
    }
    Ok(orphan_content_objects)
}

fn seen_bytes(objects: u64) -> Result<usize, ControlError> {
    usize::try_from(objects)
        .ok()
        .and_then(|count| count.checked_add(7))
        .map(|count| count / 8)
        .ok_or(ControlError::BudgetExceeded)
}

fn mark_seen(seen: &mut [u8], ordinal: u64) -> Result<bool, ControlError> {
    let zero_based = ordinal
        .checked_sub(1)
        .ok_or(ControlError::MigrationUnverified)?;
    let bit = usize::try_from(zero_based).map_err(|_| ControlError::BudgetExceeded)?;
    let byte = seen
        .get_mut(bit / 8)
        .ok_or(ControlError::MigrationUnverified)?;
    let mask = 1 << (bit % 8);
    let first = *byte & mask == 0;
    *byte |= mask;
    Ok(first)
}

fn append_blob(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ControlError> {
    let length = u32::try_from(bytes.len()).map_err(|_| ControlError::BudgetExceeded)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
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

const fn encoding_error(error: SourceContentManifestEncodingError) -> ControlError {
    match error {
        SourceContentManifestEncodingError::ObjectLimitExceeded
        | SourceContentManifestEncodingError::SourceBytesExceeded => ControlError::BudgetExceeded,
        SourceContentManifestEncodingError::InvalidHeader
        | SourceContentManifestEncodingError::InvalidState
        | SourceContentManifestEncodingError::ObjectCountMismatch => {
            ControlError::MigrationUnverified
        }
    }
}

#[cfg(test)]
#[path = "content_stage/tests.rs"]
mod tests;

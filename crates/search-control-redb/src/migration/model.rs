use search_contracts::{
    Sha256Digest32, SourceId, SourceNamespaceId, SourceRevisionId,
};

use crate::ControlError;

use super::{MAX_BYTES, MAX_ROWS, ROW_BYTES};

/// Identity of one inert mapping import, not installation or namespace ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceImportBinding {
    /// Explicit imported target namespace, never inferred from a source path.
    pub target_namespace: SourceNamespaceId,
    /// Original legacy namespace bytes, labelled SHA-256 rather than UUID/BLAKE3.
    pub legacy_namespace: Sha256Digest32,
    /// Complete verified source-event chain identity.
    pub catalog_snapshot: Sha256Digest32,
    /// Exact occurrence/identity mapping profile.
    pub mapping_profile: Sha256Digest32,
    /// Record-chain fingerprint of the corresponding canonical mapping artifact.
    pub plan_chain: Sha256Digest32,
    /// Expected input events, including retirement and path-only changes.
    pub events: u64,
    /// Expected distinct sources.
    pub sources: u64,
}

impl SourceImportBinding {
    pub(super) fn encode(self) -> Result<Vec<u8>, ControlError> {
        if self.target_namespace.as_bytes() == &[0; 16]
            || self.events > MAX_ROWS
            || self
                .events
                .checked_mul(ROW_BYTES as u64)
                .is_none_or(|bytes| bytes > MAX_BYTES)
            || self.sources > self.events
            || (self.events == 0) != (self.sources == 0)
        {
            return Err(ControlError::BudgetExceeded);
        }
        let mut out = b"ELSMAP01".to_vec();
        out.extend_from_slice(self.target_namespace.as_bytes());
        for digest in [
            self.legacy_namespace,
            self.catalog_snapshot,
            self.mapping_profile,
            self.plan_chain,
        ] {
            out.extend_from_slice(digest.as_bytes());
        }
        out.extend_from_slice(&self.events.to_be_bytes());
        out.extend_from_slice(&self.sources.to_be_bytes());
        Ok(out)
    }
}

/// Positional lifecycle flag bits for one mapping row (wire bytes 81-82).
///
/// Byte 81 packs `opens_source` (bit 0), `opens_revision` (bit 1) and
/// `retires_source` (bit 2); byte 82 carries `native_identity` as `0`/`1`.
/// The two-byte layout is frozen by fixture digests. Row-level validation
/// still rejects impossible combinations; this type only packs the bits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLifecycleFlags {
    flags: u8,
    native: u8,
}

impl SourceLifecycleFlags {
    /// Packs four lifecycle observations into the frozen two-byte wire form.
    /// Order: `[opens_source, opens_revision, retires_source, native_identity]`.
    #[must_use]
    pub const fn new(flags: [bool; 4]) -> Self {
        Self {
            flags: (flags[0] as u8)
                | ((flags[1] as u8) << 1)
                | ((flags[2] as u8) << 2),
            native: flags[3] as u8,
        }
    }

    /// True only for the source's first activation.
    #[must_use]
    pub const fn opens_source(self) -> bool {
        self.flags & 1 != 0
    }

    /// True only when a new occurrence is allocated.
    #[must_use]
    pub const fn opens_revision(self) -> bool {
        self.flags & 2 != 0
    }

    /// True for a retirement rather than an activation.
    #[must_use]
    pub const fn retires_source(self) -> bool {
        self.flags & 4 != 0
    }

    /// Legacy identity observation class; not a new native-identity qualification.
    #[must_use]
    pub const fn native_identity(self) -> bool {
        self.native != 0
    }

    /// Exact two wire bytes in row order.
    #[must_use]
    pub const fn encode(self) -> [u8; 2] {
        [self.flags, self.native]
    }
}

/// Fully typed, content-free mapping row. No arbitrary bytes or source text field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceImportRow {
    /// Global order in the verified legacy journal.
    pub sequence: u64,
    /// Proposed imported source identity.
    pub source: SourceId,
    /// Proposed occurrence identity, distinct across A/B/A activations.
    pub revision: SourceRevisionId,
    /// Prior occurrence; retained for path-only changes and retirement.
    pub previous_revision: Option<SourceRevisionId>,
    /// Per-source occurrence count, independent of global event order.
    pub occurrence: u64,
    /// Per-source event count, including non-occurrence events.
    pub source_event: u64,
    /// Exact retained source-object length.
    pub source_bytes: u64,
    /// Packed lifecycle observations; see `SourceLifecycleFlags` for the wire layout.
    pub lifecycle: SourceLifecycleFlags,
    /// Original operation identity, not a newly generated receipt.
    pub operation: Sha256Digest32,
    /// Original source identity.
    pub legacy_source: Sha256Digest32,
    /// Original content-object identity, not the occurrence ID.
    pub legacy_revision: Sha256Digest32,
    /// Actual legacy content SHA-256, never relabelled as BLAKE3.
    pub content: Sha256Digest32,
    /// Original stable-object identity fingerprint.
    pub file_identity: Sha256Digest32,
    /// Original path fingerprint, not a disclosed locator.
    pub path: Sha256Digest32,
    /// Exact source-journal event fingerprint.
    pub event: Sha256Digest32,
    /// Global predecessor event fingerprint.
    pub previous_event: Sha256Digest32,
    /// Per-source predecessor event fingerprint.
    pub previous_source_event: Sha256Digest32,
}

impl SourceImportRow {
    pub(super) fn encode(&self) -> Result<Vec<u8>, ControlError> {
        let lifecycle = self.lifecycle;
        if self.sequence == 0
            || self.sequence > MAX_ROWS
            || self.occurrence == 0
            || self.source_event == 0
            || self.source.as_bytes() == &[0; 16]
            || self.revision.as_bytes() == &[0; 16]
            || self.source_bytes > 64 * 1024 * 1024
            || (lifecycle.retires_source()
                && (lifecycle.opens_source() || lifecycle.opens_revision()))
            || (lifecycle.opens_source()
                && (!lifecycle.opens_revision()
                    || self.occurrence != 1
                    || self.source_event != 1))
            || lifecycle.opens_source() != self.previous_revision.is_none()
            || (!lifecycle.opens_revision()
                && self.previous_revision != Some(self.revision))
            || (lifecycle.opens_revision()
                && self.previous_revision == Some(self.revision))
        {
            return Err(ControlError::InvalidValue);
        }
        let mut out = Vec::with_capacity(ROW_BYTES);
        out.extend_from_slice(self.source.as_bytes());
        out.extend_from_slice(self.revision.as_bytes());
        out.push(u8::from(self.previous_revision.is_some()));
        out.extend_from_slice(
            self.previous_revision
                .as_ref()
                .map_or(&[0; 16][..], |revision| revision.as_bytes().as_slice()),
        );
        for number in [
            self.occurrence,
            self.source_event,
            self.sequence,
            self.source_bytes,
        ] {
            out.extend_from_slice(&number.to_be_bytes());
        }
        out.extend_from_slice(&lifecycle.encode());
        for digest in [
            self.operation,
            self.legacy_source,
            self.legacy_revision,
            self.content,
            self.file_identity,
            self.path,
            self.event,
            self.previous_event,
            self.previous_source_event,
        ] {
            out.extend_from_slice(digest.as_bytes());
        }
        Ok(out)
    }
}

/// Exact persisted accounting for all imported source events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceImportCounts {
    /// All source events.
    pub events: u64,
    /// Distinct mapped sources.
    pub sources: u64,
    /// New revision occurrences.
    pub occurrences: u64,
    /// Events that retain a live occurrence without retirement.
    pub retained_events: u64,
    /// Source retirements.
    pub retirements: u64,
}

impl SourceImportCounts {
    pub(super) fn encode(self, sealed: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(41);
        out.push(u8::from(sealed));
        for number in [
            self.events,
            self.sources,
            self.occurrences,
            self.retained_events,
            self.retirements,
        ] {
            out.extend_from_slice(&number.to_be_bytes());
        }
        out
    }

    pub(super) fn add(&mut self, row: &SourceImportRow) {
        // Validated sequence/count bounds are at most MAX_ROWS.
        let lifecycle = row.lifecycle;
        self.events += 1;
        self.sources += u64::from(lifecycle.opens_source());
        self.occurrences += u64::from(lifecycle.opens_revision());
        self.retained_events +=
            u64::from(!lifecycle.opens_revision() && !lifecycle.retires_source());
        self.retirements += u64::from(lifecycle.retires_source());
    }
}

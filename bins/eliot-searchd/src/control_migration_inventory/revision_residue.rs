//! Ordered residue tuples from one complete physical revision traversal.
//! Parent custody is retained; rows are pass-local history, never read authority.
use super::{Family, LegacyInventoryEncoder, LegacyInventoryLimits, MigrationSource, Status, Work, io};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Row {
    pub(super) status: Status,
    pub(super) kind: u16,
    pub(super) refs: [[u8; 32]; 3],
    pub(super) counts: [u64; 3],
}

pub(super) struct View {
    rows: Vec<Row>,
    encoded_bytes: u64,
    _scope: io::ParentReadScope,
}

pub(super) struct Capture {
    rows: Vec<Row>,
    slots: usize,
    current_records: u64,
    current_bytes: u64,
    residue_records: u64,
    residue_bytes: u64,
}

/// Private single-use admission, including unfinished directory outcomes.
pub(super) struct Reservation {
    retain: bool,
    current_bytes: u64,
    residue_bytes: u64,
}

impl Capture {
    pub(super) fn new(source: &MigrationSource<'_, '_, '_>) -> Result<Self, String> {
        source.charge(Work::Allocate(std::mem::size_of::<Self>() + std::mem::size_of::<View>()))?;
        Ok(Self { rows: Vec::new(), slots: 0, current_records: 0, current_bytes: 0,
            residue_records: 0, residue_bytes: 0 })
    }

    pub(super) fn reserve(&mut self, source: &MigrationSource<'_, '_, '_>,
        encoder: &LegacyInventoryEncoder, retain: bool,
    ) -> Result<Reservation, String> {
        // Every revision physical row has kind <= 36 and only its first count
        // can vary. Use owner sizing before any dependent read or enumeration.
        let bound = |family| LegacyInventoryEncoder::item_encoded_len(
            family, Status::AbsentProven, 36, [u64::MAX, 0, 0])
            .map_err(super::codec_reason).and_then(|bytes| u64::try_from(bytes).map_err(|_| limit()));
        let current_bytes = bound(Family::Revisions)?;
        let residue_bytes = if retain { bound(Family::Residue)? } else { 0 };
        let current_records = self.current_records.checked_add(1).ok_or_else(limit)?;
        let current_total = self.current_bytes.checked_add(current_bytes).ok_or_else(limit)?;
        let residue_records = self.residue_records.checked_add(u64::from(retain)).ok_or_else(limit)?;
        let residue_total = self.residue_bytes.checked_add(residue_bytes).ok_or_else(limit)?;
        encoder.preflight_remaining(current_records.checked_add(residue_records).ok_or_else(limit)?,
            current_total.checked_add(residue_total).ok_or_else(limit)?)
            .map_err(super::codec_reason)?;
        if retain {
            let needed = self.rows.len().checked_add(self.slots).and_then(|n| n.checked_add(1)).ok_or_else(limit)?;
            let maximum = usize::try_from(LegacyInventoryLimits::BASELINE.records).map_err(|_| limit())?;
            if needed > maximum { return Err(limit()); }
            if needed > self.rows.capacity() {
                let capacity = self.rows.capacity().saturating_mul(2).max(8).min(maximum).max(needed);
                source.charge(Work::Allocate(capacity.checked_mul(std::mem::size_of::<Row>()).ok_or_else(limit)?))?;
                source.charge(Work::Compare(self.rows.len().checked_mul(std::mem::size_of::<Row>()).ok_or_else(limit)?))?;
                self.rows.try_reserve_exact(capacity - self.rows.len()).map_err(|_| limit())?;
            }
            source.charge(Work::Compare(std::mem::size_of::<Row>()))?;
            self.slots += 1;
        }
        self.current_records = current_records;
        self.current_bytes = current_total;
        self.residue_records = residue_records;
        self.residue_bytes = residue_total;
        Ok(Reservation { retain, current_bytes, residue_bytes })
    }

    #[expect(clippy::needless_pass_by_value, reason = "Consume a single-use pre-I/O reservation; a borrow would allow reuse")]
    pub(super) fn complete(&mut self, slot: Reservation, row: Option<Row>) -> Result<(), String> {
        if slot.retain {
            let row = row.ok_or_else(changed)?;
            let bytes = u64::try_from(LegacyInventoryEncoder::item_encoded_len(
                Family::Residue, row.status, row.kind, row.counts).map_err(super::codec_reason)?)
                .map_err(|_| limit())?;
            if bytes > slot.residue_bytes { return Err(changed()); }
            self.residue_bytes = self.residue_bytes.checked_sub(slot.residue_bytes)
                .and_then(|n| n.checked_add(bytes)).ok_or_else(changed)?;
            self.slots = self.slots.checked_sub(1).ok_or_else(changed)?;
            // Capacity and copy admission belong to reserve, before dependent I/O.
            self.rows.push(row);
        }
        self.current_records = self.current_records.checked_sub(1).ok_or_else(changed)?;
        self.current_bytes = self.current_bytes.checked_sub(slot.current_bytes).ok_or_else(changed)?;
        Ok(())
    }

    pub(super) fn finish(self, scope: io::ParentReadScope) -> Result<View, String> {
        if self.slots != 0 || self.current_records != 0 || self.current_bytes != 0
            || u64::try_from(self.rows.len()).map_err(|_| limit())? != self.residue_records {
            return Err(changed());
        }
        Ok(View { rows: self.rows, encoded_bytes: self.residue_bytes, _scope: scope })
    }
}

impl View {
    pub(super) fn preflight(&self, encoder: &LegacyInventoryEncoder) -> Result<(), String> {
        encoder.preflight_remaining(u64::try_from(self.rows.len()).map_err(|_| limit())?, self.encoded_bytes)
            .map_err(super::codec_reason)?;
        Ok(())
    }

    pub(super) fn project(&self, source: &MigrationSource<'_, '_, '_>,
        mut emit: impl FnMut(Row) -> Result<(), String>,
    ) -> Result<(), String> {
        source.charge(Work::Compare(self.rows.len().checked_mul(std::mem::size_of::<Row>()).ok_or_else(limit)?))?;
        for row in &self.rows { emit(*row)?; }
        source.poll()
    }
}

fn changed() -> String { "DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned() }
fn limit() -> String { "DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned() }

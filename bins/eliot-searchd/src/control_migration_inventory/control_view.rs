//! One bounded physical control observation per source pass, never current authority.
//! Bodies are decoded while borrowed and discarded; only existing inventory tuples remain.

use super::{Family, LegacyInventoryLimits, MigrationSource, Status, Work, io};
use std::path::Path;

const ZERO: [u8; 32] = [0; 32];
const RESIDUE: u8 = 1;
const OPERATIONS: u8 = 2;
const PUBLICATIONS: u8 = 4;
const SECURITY: u8 = 8;
const ALL: u8 = RESIDUE | OPERATIONS | PUBLICATIONS | SECURITY;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Row {
    pub(super) status: Status,
    pub(super) kind: u16,
    pub(super) refs: [[u8; 32]; 3],
    pub(super) counts: [u64; 3],
}

struct Event {
    families: u8,
    row: Row,
    // Only nested unsupported files derive this count from the projected family.
    residue_orphan: bool,
}

pub(super) struct ControlView {
    events: Vec<Event>,
    projected_rows: u64,
}

impl ControlView {
    pub(super) fn capture(
        source: &MigrationSource<'_, '_, '_>,
        root: &Path,
    ) -> Result<Self, String> {
        if root != source.inventory_root() {
            return Err(changed());
        }
        source.checkpoint()?;
        let mut view = Self {
            events: Vec::new(),
            projected_rows: 0,
        };
        let directory = root.join("control");
        let outcome = io::visit(source, &directory, |name| {
            let text = name.value.to_str().unwrap_or("");
            let residue = Path::new(text).extension().is_some_and(|extension| {
                ["tmp", "bak", "pending"]
                    .iter()
                    .any(|suffix| extension.eq_ignore_ascii_case(suffix))
            });
            let mut mask = if residue { RESIDUE } else { 0 };
            if text.contains("operation") || text.contains("receipt") || text.contains("intent") {
                mask |= OPERATIONS;
            }
            if text.contains("publication")
                || text.contains("manifest")
                || text.contains("shadow")
                || text.contains("epoch")
                || text == "control-cutover.v1"
            {
                mask |= PUBLICATIONS;
            }
            if text.contains("purge") || text.contains("quarantine") || text.contains("retention") {
                mask |= SECURITY;
            }
            let unknown = !residue
                && !matches!(
                    text,
                    "namespace.id"
                        | "source-events.log"
                        | "source-roots.v1"
                        | "directory-manifests"
                        | "control-cutover.v1"
                );
            if mask == 0 && !unknown {
                return Ok(());
            }
            let subject = io::child_locator(source, &directory, &name.value)?;
            if unknown {
                // Another family's physical observation cannot promote this missing owner/parser.
                view.push(
                    source,
                    RESIDUE,
                    row(Status::Gap, 34, subject, [0; 3]),
                    false,
                )?;
            }
            if mask == 0 {
                return Ok(());
            }
            let path = directory.join(&name.value);
            let observed = io::metadata(source, &path)?;
            match observed {
                io::Observed::Object(meta) if meta.is_dir() => {
                    view.push(
                        source,
                        mask,
                        row(Status::Gap, 35, subject, [meta.len(), 0, 0]),
                        false,
                    )?;
                    view.directory(source, &path, mask, 0)
                }
                observed => {
                    let value = file(source, &path, subject, observed, residue)?;
                    view.push(source, mask, value, false)
                }
            }
        })?;
        view.outcome(source, &directory, ALL, outcome)?;
        source.checkpoint()?;
        Ok(view)
    }

    pub(super) fn project(
        &self,
        source: &MigrationSource<'_, '_, '_>,
        family: Family,
        mut emit: impl FnMut(Row) -> Result<(), String>,
    ) -> Result<(), String> {
        let mask = match family {
            Family::Residue => RESIDUE,
            Family::Operations => OPERATIONS,
            Family::Publications => PUBLICATIONS,
            Family::Security => SECURITY,
            _ => return Err(changed()),
        };
        for event in &self.events {
            source.charge(Work::Compare(std::mem::size_of::<Event>()))?;
            if event.families & mask != 0 {
                let mut value = event.row;
                if event.residue_orphan {
                    value.counts[2] = u64::from(mask == RESIDUE);
                }
                emit(value)?;
            }
        }
        source.poll()
    }

    fn directory(
        &mut self,
        source: &MigrationSource<'_, '_, '_>,
        directory: &Path,
        mask: u8,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 3 {
            return self.push(
                source,
                mask,
                row(
                    Status::Unsupported,
                    14,
                    io::locator(source, directory)?,
                    [depth as u64, 0, 0],
                ),
                false,
            );
        }
        let outcome = io::visit(source, directory, |name| {
            let path = directory.join(&name.value);
            let subject = io::locator(source, &path)?;
            let observed = io::metadata(source, &path)?;
            match observed {
                io::Observed::Object(meta) if meta.is_dir() => {
                    self.push(
                        source,
                        mask,
                        row(Status::Gap, 35, subject, [meta.len(), depth as u64 + 1, 0]),
                        false,
                    )?;
                    self.directory(source, &path, mask, depth + 1)
                }
                observed => {
                    // Nested basenames do not select families or admit body decoders.
                    let value = unsupported(subject, observed, false);
                    self.push(source, mask, value, true)
                }
            }
        })?;
        self.outcome(source, directory, mask, outcome)
    }

    fn outcome(
        &mut self,
        source: &MigrationSource<'_, '_, '_>,
        directory: &Path,
        mask: u8,
        outcome: Option<Status>,
    ) -> Result<(), String> {
        let (status, kind) = outcome.map_or((Status::Present, 36), |status| (status, 16));
        self.push(
            source,
            mask,
            row(status, kind, io::locator(source, directory)?, [0; 3]),
            false,
        )
    }

    fn push(
        &mut self,
        source: &MigrationSource<'_, '_, '_>,
        families: u8,
        row: Row,
        residue_orphan: bool,
    ) -> Result<(), String> {
        let projected = self
            .projected_rows
            .checked_add(u64::from(families.count_ones()))
            .ok_or_else(limit)?;
        if projected > LegacyInventoryLimits::BASELINE.records {
            return Err(limit());
        }
        if self.events.len() == self.events.capacity() {
            let capacity = self.events.capacity().saturating_mul(2).max(8);
            let bytes = capacity
                .checked_mul(std::mem::size_of::<Event>())
                .ok_or_else(limit)?;
            if bytes as u64 > LegacyInventoryLimits::BASELINE.encoded_bytes {
                return Err(limit());
            }
            source.charge(Work::Allocate(bytes))?;
            self.events
                .try_reserve_exact(capacity - self.events.len())
                .map_err(|_| limit())?;
        }
        source.charge(Work::Compare(std::mem::size_of::<Event>()))?;
        self.events.push(Event {
            families,
            row,
            residue_orphan,
        });
        self.projected_rows = projected;
        Ok(())
    }
}

fn row(status: Status, kind: u16, subject: [u8; 32], counts: [u64; 3]) -> Row {
    Row {
        status,
        kind,
        refs: [subject, ZERO, ZERO],
        counts,
    }
}

fn unsupported(subject: [u8; 32], observed: io::Observed, orphan: bool) -> Row {
    let (status, size) = match observed {
        io::Observed::Missing => (Status::Gap, 0),
        io::Observed::Refused(status) => (status, 0),
        io::Observed::Object(meta) => (Status::Unsupported, meta.len()),
    };
    row(status, 14, subject, [size, 0, u64::from(orphan)])
}

fn file(
    source: &MigrationSource<'_, '_, '_>,
    path: &Path,
    subject: [u8; 32],
    observed: io::Observed,
    orphan: bool,
) -> Result<Row, String> {
    let supported = path
        .strip_prefix(source.inventory_root())
        .is_ok_and(|relative| {
            let mut parts = relative.components();
            parts.next()
                == Some(std::path::Component::Normal(std::ffi::OsStr::new(
                    "control",
                )))
                && parts.next()
                    == Some(std::path::Component::Normal(std::ffi::OsStr::new(
                        "control-cutover.v1",
                    )))
                && parts.next().is_none()
        });
    if !supported {
        return Ok(unsupported(subject, observed, orphan));
    }
    match io::read(source, path, io::CONTROL_BYTES)? {
        Ok(bytes) => {
            let fingerprint = io::fingerprint(source, &bytes)?;
            Ok(
                match search_control_redb::migration::ControlCutoverMarker::decode(&bytes) {
                    Ok(marker) => Row {
                        status: Status::Present,
                        kind: 15,
                        refs: [subject, fingerprint, marker.catalog_snapshot],
                        counts: [marker.epoch, bytes.len() as u64, 0],
                    },
                    Err(_) => Row {
                        status: Status::Corrupt,
                        kind: 15,
                        refs: [subject, fingerprint, ZERO],
                        counts: [bytes.len() as u64, 0, 0],
                    },
                },
            )
        }
        Err(status) => Ok(row(
            if status == Status::AbsentProven {
                Status::Gap
            } else {
                status
            },
            14,
            subject,
            [0; 3],
        )),
    }
}

fn changed() -> String {
    "DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned()
}
fn limit() -> String {
    "DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned()
}

#[cfg(all(test, windows))]
#[path = "control_view_tests.rs"]
mod tests;

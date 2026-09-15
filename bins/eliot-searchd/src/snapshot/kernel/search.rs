use std::io::{self, ErrorKind};

use super::model::{SnapshotIndex, SnapshotMatch, SnapshotSearchResult};
use super::spec::FINGERPRINT_ALGORITHM;
use super::storage::read_verified_revision;

impl SnapshotIndex {
    /// Reopens exact retained revisions and performs a bounded DIRECT search.
    pub(crate) fn search(&self, query: &str) -> io::Result<SnapshotSearchResult> {
        if query.is_empty() || query.len() > 1_024 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "query must be non-empty and at most 1024 UTF-8 bytes",
            ));
        }
        let mut matches = Vec::new();
        let mut scanned_revisions = 0_usize;
        let mut unavailable_revisions = 0_usize;
        let mut truncated = false;

        for entry in &self.entries {
            if matches.len() >= self.limits.results {
                truncated = true;
                break;
            }
            let Ok(bytes) = read_verified_revision(
                &entry.revision_path,
                entry.revision_fingerprint,
                entry.byte_length,
                self.limits.file_bytes,
            ) else {
                unavailable_revisions = unavailable_revisions.saturating_add(1);
                continue;
            };
            scanned_revisions = scanned_revisions.saturating_add(1);
            let text = String::from_utf8(bytes).map_err(|_| {
                io::Error::new(ErrorKind::InvalidData, "retained revision is not UTF-8")
            })?;
            for line in lines_with_offsets(&text) {
                let Some(column) = find_query(line.text, query) else {
                    continue;
                };
                let byte_start = line
                    .byte_start
                    .checked_add(column)
                    .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "offset overflow"))?;
                let byte_end = byte_start
                    .checked_add(query.len())
                    .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "offset overflow"))?;
                matches.push(SnapshotMatch {
                    root_index: entry.root_index,
                    relative_path: entry.relative_path.clone(),
                    revision_fingerprint: entry.revision_fingerprint,
                    line: line.number.saturating_add(1),
                    column_bytes: column,
                    byte_start,
                    byte_end,
                    excerpt: truncate_chars(line.text.trim(), self.limits.excerpt_chars),
                });
                if matches.len() >= self.limits.results {
                    truncated = true;
                    break;
                }
            }
        }

        Ok(SnapshotSearchResult {
            snapshot_id: self.snapshot_id.clone(),
            manifest_fingerprint: self.manifest_fingerprint,
            fingerprint_algorithm: FINGERPRINT_ALGORITHM,
            matches,
            scanned_revisions,
            unavailable_revisions,
            denominator_files: self.entries.len(),
            complete: !truncated && !self.stats.truncated && unavailable_revisions == 0,
            truncated,
        })
    }
}

#[derive(Clone, Copy)]
struct LineRef<'a> {
    number: usize,
    byte_start: usize,
    text: &'a str,
}

fn lines_with_offsets(text: &str) -> Vec<LineRef<'_>> {
    if text.is_empty() {
        return Vec::new();
    }
    let bytes = text.as_bytes();
    let mut output = Vec::new();
    let mut start = 0_usize;
    let mut number = 0_usize;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte != b'\n' {
            continue;
        }
        let mut end = index;
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        output.push(LineRef {
            number,
            byte_start: start,
            text: &text[start..end],
        });
        start = index.saturating_add(1);
        number = number.saturating_add(1);
    }
    if start < text.len() {
        output.push(LineRef {
            number,
            byte_start: start,
            text: &text[start..],
        });
    }
    output
}

fn find_query(line: &str, query: &str) -> Option<usize> {
    if let Some(offset) = line.find(query) {
        return Some(offset);
    }
    if !query.is_ascii() {
        return None;
    }
    let query = query.as_bytes();
    if query.len() > line.len() {
        return None;
    }
    line.as_bytes()
        .windows(query.len())
        .position(|window| window.eq_ignore_ascii_case(query))
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut output = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        output.push('…');
    }
    output
}

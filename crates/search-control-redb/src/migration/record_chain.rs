//! Canonical bounded SHA-256 record chain for inactive source-import artifacts.
//!
//! Mapping plans and source-content manifests share this frozen chain profile.
//! The chain authenticates record order and exact newline-terminated bytes; it
//! is not a raw-file SHA-256, an authority receipt or a live control record.

use core::fmt;

use sha2::{Digest, Sha256};

const PARTS_V1: &[u8] = b"eliot-search/sha256-parts/v1\0";
const CHAIN_DOMAIN: &[u8] = b"eliot-search/source-map-chain/v1";
const ROW_DOMAIN: &[u8] = b"eliot-search/source-map-row/v1";
const END_DOMAIN: &[u8] = b"eliot-search/source-map-end/v1";

/// Maximum encoded bytes in one inactive source-import record artifact.
pub const MAX_SOURCE_IMPORT_RECORD_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum bytes in one newline-terminated source-import record.
pub const MAX_SOURCE_IMPORT_ROW_BYTES: usize = 8 * 1024;

/// Closed record-chain failure preserving the historical migration reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceImportRecordChainError {
    /// One row exceeds the fixed per-record ceiling.
    RowTooLarge,
    /// Aggregate bytes or row count cannot advance within bounds.
    ArtifactTooLarge,
    /// A row is not exactly one nonempty newline-terminated record.
    RowInvalid,
}

impl SourceImportRecordChainError {
    /// Stable historical daemon reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RowTooLarge => "DIRECT_MIGRATION_PLAN_ROW_TOO_LARGE",
            Self::ArtifactTooLarge => "DIRECT_MIGRATION_PLAN_TOO_LARGE",
            Self::RowInvalid => "DIRECT_MIGRATION_PLAN_ROW_INVALID",
        }
    }
}

impl fmt::Display for SourceImportRecordChainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SourceImportRecordChainError {}

/// Stateful canonical record-chain accumulator.
#[derive(Clone, Debug)]
pub struct SourceImportRecordChain {
    chain: [u8; 32],
    rows: u64,
    bytes: u64,
}

impl SourceImportRecordChain {
    /// Starts the frozen empty record-chain state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            chain: digest_parts(CHAIN_DOMAIN, &[]),
            rows: 0,
            bytes: 0,
        }
    }

    /// Adds one exact newline-terminated record.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an oversized row/artifact, row-count
    /// exhaustion, an empty record or embedded/missing newlines.
    pub fn push(
        &mut self,
        row: &[u8],
    ) -> Result<(), SourceImportRecordChainError> {
        if row.len() > MAX_SOURCE_IMPORT_ROW_BYTES {
            return Err(SourceImportRecordChainError::RowTooLarge);
        }
        if row.is_empty()
            || row.last() != Some(&b'\n')
            || row[..row.len() - 1].contains(&b'\n')
        {
            return Err(SourceImportRecordChainError::RowInvalid);
        }
        let bytes = self
            .bytes
            .checked_add(
                u64::try_from(row.len())
                    .map_err(|_| SourceImportRecordChainError::ArtifactTooLarge)?,
            )
            .filter(|total| *total <= MAX_SOURCE_IMPORT_RECORD_BYTES)
            .ok_or(SourceImportRecordChainError::ArtifactTooLarge)?;
        let rows = self
            .rows
            .checked_add(1)
            .ok_or(SourceImportRecordChainError::ArtifactTooLarge)?;
        self.chain = digest_parts(
            ROW_DOMAIN,
            &[&self.chain, &rows.to_be_bytes(), row],
        );
        self.rows = rows;
        self.bytes = bytes;
        Ok(())
    }

    /// Number of accepted records.
    #[must_use]
    pub const fn rows(&self) -> u64 {
        self.rows
    }

    /// Number of accepted encoded bytes.
    #[must_use]
    pub const fn encoded_bytes(&self) -> u64 {
        self.bytes
    }

    /// Finishes the chain with exact row/byte cardinality.
    #[must_use]
    pub fn finish(self) -> [u8; 32] {
        digest_parts(
            END_DOMAIN,
            &[
                &self.chain,
                &self.rows.to_be_bytes(),
                &self.bytes.to_be_bytes(),
            ],
        )
    }
}

impl Default for SourceImportRecordChain {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PARTS_V1);
    hasher.update(
        u64::try_from(domain.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(domain);
    hasher.update(
        u64::try_from(parts.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for part in parts {
        hasher.update(
            u64::try_from(part.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests;

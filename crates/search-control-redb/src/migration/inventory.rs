//! Versioned streamed legacy inventory. Accounting is never migration authority.

use core::fmt;
use search_contracts::{Sha256Digest32, SourceNamespaceId};
use sha2::{Digest, Sha256};
use super::SourceImportRow;

/// Independent inventory wire profile; existing mapping profiles are unchanged.
pub const LEGACY_INVENTORY_PROFILE: &str = "eliot.legacy-inventory.v1";
/// Number of mandatory family terminals.
pub const LEGACY_INVENTORY_FAMILIES: usize = 12;

/// Closed, ordered denominators of the complete inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum LegacyInventoryFamily {
    /// Installation, physical root and retained owner.
    Owner,
    /// Namespace, every source event, occurrence and head.
    Sources,
    /// Exact registration and registered root references.
    Roots,
    /// Every immutable directory generation.
    DirectoryManifests,
    /// All retained revisions and physical encoding observations.
    Revisions,
    /// Preparation, representations, units and profile references.
    Preparation,
    /// Physical temporary, orphan, dangling and unknown objects.
    Residue,
    /// Operation/idempotency evidence and missing original linkage.
    Operations,
    /// Publication, manifest, shadow and epoch references.
    Publications,
    /// Purge, quarantine and retention references.
    Security,
    /// Existing credential/key/profile references, never keys.
    Credentials,
    /// Complete inventory limit, paging and profile identity.
    Limits,
}

impl LegacyInventoryFamily {
    /// Fixed order; a footer requires every member exactly once.
    pub const ALL: [Self; LEGACY_INVENTORY_FAMILIES] = [Self::Owner, Self::Sources,
        Self::Roots, Self::DirectoryManifests, Self::Revisions, Self::Preparation,
        Self::Residue, Self::Operations, Self::Publications, Self::Security,
        Self::Credentials, Self::Limits];
    /// Canonical family tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Owner => "owner", Self::Sources => "sources", Self::Roots => "roots",
            Self::DirectoryManifests => "directory-manifests", Self::Revisions => "revisions",
            Self::Preparation => "preparation", Self::Residue => "residue",
            Self::Operations => "operations", Self::Publications => "publications",
            Self::Security => "security", Self::Credentials => "credentials", Self::Limits => "limits",
        }
    }
}

/// Exact closed family/item disposition. No unknown state is proven absence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum LegacyInventoryStatus {
    /// Exact readable evidence was observed.
    Present,
    /// An explicitly bounded, fully observed locator denominator is empty.
    AbsentProven,
    /// Evidence exists but its representation/semantics are unsupported.
    Unsupported,
    /// The accepted owning parser rejected evidence.
    Corrupt,
    /// Required observation could not be read.
    Unreadable,
    /// Physical evidence has no accepted current reference.
    Orphan,
    /// Required authority/linkage/coverage is unavailable.
    Gap,
}

impl LegacyInventoryStatus {
    /// Canonical outcome tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self { Self::Present => "present", Self::AbsentProven => "absent-proven",
            Self::Unsupported => "unsupported", Self::Corrupt => "corrupt",
            Self::Unreadable => "unreadable", Self::Orphan => "orphan", Self::Gap => "gap" }
    }
}

/// Content-free original operation observation supplied by retained native custody.
/// This value grants no recovery/cutover authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyInventoryOwner {
    /// Existing installation ID.
    pub installation: [u8; 16],
    /// Existing installation incarnation.
    pub incarnation: [u8; 16],
    /// Existing data-root ID.
    pub root: [u8; 16],
    /// Original operation invocation ID, not the startup request of another child.
    pub invocation: [u8; 16],
    /// Exact original native request BLAKE3 digest.
    pub request_blake3: [u8; 32],
    /// Retained durable owner record BLAKE3 digest.
    pub record_blake3: [u8; 32],
    /// Verified physical-root binding BLAKE3 digest.
    pub physical_blake3: [u8; 32],
    /// Retained owner epoch; this observation does not acquire ACTIVE authority.
    pub epoch: u64,
    /// Retained owner record generation.
    pub generation: u64,
}

/// Fixed finite inventory limits beneath the original native allowance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyInventoryLimits {
    /// Maximum canonical records including all terminals and footer.
    pub records: u64,
    /// Maximum encoded artifact bytes.
    pub encoded_bytes: u64,
    /// Maximum names retained in one deterministic directory page.
    pub page_names: usize,
}

impl LegacyInventoryLimits {
    /// Version-one hard ceilings; exhausting one refuses publication.
    pub const BASELINE: Self = Self { records: 2_000_000,
        encoded_bytes: 512 * 1024 * 1024, page_names: 32 };
    /// Rejects zero, inflated and footer-insufficient limits.
    /// # Errors
    /// Returns `Limit` when a component ceiling is invalid.
    pub const fn validate(self) -> Result<Self, LegacyInventoryError> {
        if self.records < 14 || self.records > Self::BASELINE.records
            || self.encoded_bytes == 0 || self.encoded_bytes > Self::BASELINE.encoded_bytes
            || self.page_names == 0 || self.page_names > Self::BASELINE.page_names {
            Err(LegacyInventoryError::Limit)
        } else { Ok(self) }
    }
}

/// Immutable header inputs; no raw path, source body or key field is representable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyInventoryBinding {
    /// Original retained operation/root observation.
    pub owner: LegacyInventoryOwner,
    /// Explicit import target.
    pub target: SourceNamespaceId,
    /// Actual legacy namespace SHA-256 identity.
    pub namespace: Sha256Digest32,
    /// Exact original legacy source snapshot SHA-256.
    pub snapshot: Sha256Digest32,
    /// Exact retained registration bytes SHA-256 and length, or pinned absence.
    pub registration: Option<(Sha256Digest32, u64)>,
}

/// Fixed-size stream accounting, not an inventory collection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LegacyInventorySummary {
    /// Encoded records, including header, all terminals and footer.
    pub records: u64,
    /// Encoded artifact bytes.
    pub encoded_bytes: u64,
    /// Per-family item counts by the seven closed statuses.
    pub counts: [[u64; 7]; LEGACY_INVENTORY_FAMILIES],
    /// Fully observed family denominators; gaps remain explicit.
    pub denominators: [bool; LEGACY_INVENTORY_FAMILIES],
}

/// Closed content-free codec/accounting refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyInventoryError {
    /// A row/item/byte/page bound was exceeded.
    Limit,
    /// Family order, terminal or absence classification was contradictory.
    Accounting,
    /// Metadata failed its existing owning encoder.
    Metadata,
}
impl fmt::Display for LegacyInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self { Self::Limit => "LEGACY_INVENTORY_LIMIT",
            Self::Accounting => "LEGACY_INVENTORY_ACCOUNTING", Self::Metadata => "LEGACY_INVENTORY_METADATA" })
    }
}
impl std::error::Error for LegacyInventoryError {}

/// One bounded encoder. Each returned row must be streamed before the next call.
pub struct LegacyInventoryEncoder {
    limits: LegacyInventoryLimits,
    summary: LegacyInventorySummary,
    family: usize,
    started: bool,
}

impl LegacyInventoryEncoder {
    /// Starts fixed-size accounting under validated component limits.
    /// # Errors
    /// Returns `Limit` for rejected component limits.
    pub fn new(limits: LegacyInventoryLimits) -> Result<Self, LegacyInventoryError> {
        Ok(Self { limits: limits.validate()?, summary: LegacyInventorySummary::default(),
            family: 0, started: false })
    }
    /// Canonical header; identities retain their declared digest algorithms.
    /// # Errors
    /// Returns `Accounting` for repeated headers or invalid owner observations,
    /// and `Limit` for exhausted output bounds.
    pub fn header(&mut self, b: LegacyInventoryBinding) -> Result<Vec<u8>, LegacyInventoryError> {
        if self.started || b.owner.epoch == 0 { return Err(LegacyInventoryError::Accounting); }
        self.started = true;
        let o = b.owner;
        let registration = b.registration.map_or_else(|| "absent".to_owned(),
            |(d, n)| format!("sha256:{}:{n}", hex(d.as_bytes())));
        self.row(format!("H\t{LEGACY_INVENTORY_PROFILE}\t{}\t{}\t{}\t{}\tblake3:{}\tblake3:{}\tblake3:{}\t{}\t{}\t{}\tsha256:{}\tsha256:{}\t{registration}\t{}\t{}\t{}\tauthority=none\n",
            hex(&o.installation), hex(&o.incarnation), hex(&o.root), hex(&o.invocation),
            hex(&o.request_blake3), hex(&o.record_blake3), hex(&o.physical_blake3),
            o.epoch, o.generation, b.target, hex(b.namespace.as_bytes()), hex(b.snapshot.as_bytes()),
            self.limits.records, self.limits.encoded_bytes, self.limits.page_names))
    }
    /// Encodes bounded content-free references and measured counts. Kind is numeric,
    /// with its meaning frozen by the profile, never an arbitrary input string.
    /// # Errors
    /// Returns `Accounting` for an out-of-order family and `Limit` for exhaustion.
    pub fn item(&mut self, family: LegacyInventoryFamily, status: LegacyInventoryStatus,
        kind: u16, refs: [Sha256Digest32; 3], counts: [u64; 3]) -> Result<Vec<u8>, LegacyInventoryError> {
        self.account(family, status)?;
        self.row(format!("I\t{}\t{}\t{kind}\t{}\t{}\t{}\t{}\t{}\t{}\n", family.tag(), status.tag(),
            hex(refs[0].as_bytes()), hex(refs[1].as_bytes()), hex(refs[2].as_bytes()),
            counts[0], counts[1], counts[2]))
    }
    /// Reuses the accepted content-free mapping-row encoder for every source event.
    /// # Errors
    /// Returns `Metadata` for a rejected source row, `Accounting` for wrong order
    /// or `Limit` when encoded bounds are exhausted.
    pub fn source_event(&mut self, source: &SourceImportRow) -> Result<Vec<u8>, LegacyInventoryError> {
        let bytes = source.encode().map_err(|_| LegacyInventoryError::Metadata)?;
        self.account(LegacyInventoryFamily::Sources, LegacyInventoryStatus::Present)?;
        self.row(format!("E\tsources\tpresent\t{}\n", hex(&bytes)))
    }
    /// Ends exactly the next family. Proven absence requires a frozen observed
    /// denominator; pagination alone cannot supply this flag.
    /// # Errors
    /// Returns `Accounting` for missing/out-of-order terminals or false absence,
    /// and `Limit` when output bounds are exhausted.
    pub fn end_family(&mut self, family: LegacyInventoryFamily, terminal: LegacyInventoryStatus,
        denominator_complete: bool) -> Result<Vec<u8>, LegacyInventoryError> {
        if !self.started || self.family != family as usize
            || (terminal == LegacyInventoryStatus::AbsentProven && (!denominator_complete
                || self.summary.counts[self.family].iter().enumerate()
                    .any(|(i, n)| i != LegacyInventoryStatus::AbsentProven as usize && *n != 0))) {
            return Err(LegacyInventoryError::Accounting);
        }
        self.summary.denominators[self.family] = denominator_complete;
        let counts = self.summary.counts[self.family];
        self.family += 1;
        self.row(format!("T\t{}\t{}\t{}\t{},{},{},{},{},{},{}\n", family.tag(), terminal.tag(),
            u8::from(denominator_complete), counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6]))
    }
    /// A footer exists only after all twelve terminals. It proves accounting only.
    /// # Errors
    /// Returns `Accounting` for incomplete families and `Limit` for exhaustion.
    pub fn finish(mut self) -> Result<(Vec<u8>, LegacyInventorySummary), LegacyInventoryError> {
        if self.family != LEGACY_INVENTORY_FAMILIES { return Err(LegacyInventoryError::Accounting); }
        let row = self.row(format!("F\t{LEGACY_INVENTORY_PROFILE}\t12\t{}\t{}\trecovery=unresolved\tcutover=none\n",
            self.summary.records, self.summary.encoded_bytes))?;
        Ok((row, self.summary))
    }
    fn account(&mut self, family: LegacyInventoryFamily, status: LegacyInventoryStatus) -> Result<(), LegacyInventoryError> {
        if !self.started || self.family != family as usize { return Err(LegacyInventoryError::Accounting); }
        let count = &mut self.summary.counts[self.family][status as usize];
        *count = count.checked_add(1).ok_or(LegacyInventoryError::Limit)?;
        Ok(())
    }
    fn row(&mut self, row: String) -> Result<Vec<u8>, LegacyInventoryError> {
        if row.len() > super::MAX_SOURCE_IMPORT_ROW_BYTES { return Err(LegacyInventoryError::Limit); }
        self.summary.records = self.summary.records.checked_add(1).ok_or(LegacyInventoryError::Limit)?;
        self.summary.encoded_bytes = self.summary.encoded_bytes.checked_add(row.len() as u64)
            .ok_or(LegacyInventoryError::Limit)?;
        if self.summary.records > self.limits.records || self.summary.encoded_bytes > self.limits.encoded_bytes {
            return Err(LegacyInventoryError::Limit);
        }
        Ok(row.into_bytes())
    }
}

/// Exact new profile descriptor digest; existing chain/row domains are untouched.
#[must_use]
pub fn legacy_inventory_profile_digest() -> Sha256Digest32 {
    Sha256Digest32::from_bytes(Sha256::digest(b"eliot.legacy-inventory.v1\0H,I,E,T,F;12-families;7-statuses;page32;row8192;rows2000000;bytes536870912;authority-none;recovery-unresolved").into())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes { text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 15)])); }
    text
}

#[cfg(test)]
mod tests;

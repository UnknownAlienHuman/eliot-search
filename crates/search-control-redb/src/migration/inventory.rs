//! Versioned streamed legacy inventory. Accounting is never migration authority.

use super::SourceImportRow;
use core::fmt;
use search_contracts::{Sha256Digest32, SourceNamespaceId};
use sha2::{Digest, Sha256};

mod decoder;

pub use decoder::{LegacyInventoryDecodedRow, LegacyInventoryDecoder};

/// Independent inventory wire profile; existing mapping profiles are unchanged.
pub const LEGACY_INVENTORY_PROFILE: &str = "eliot.legacy-inventory.v1";
/// Explicit wire semantics; old basename evidence is never a V2 locator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyInventoryProfile {
    /// Frozen original basename-reference profile.
    V1,
    /// Root-relative, component-framed native physical locator references.
    LocationV2,
}

impl LegacyInventoryProfile {
    /// Exact header/footer profile name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::V1 => LEGACY_INVENTORY_PROFILE,
            Self::LocationV2 => "eliot.legacy-inventory.v2",
        }
    }
    /// Exact suffix of the original published inventory artifact.
    #[must_use]
    pub const fn artifact_suffix(self) -> &'static str {
        match self {
            Self::V1 => "legacy-inventory.v1",
            Self::LocationV2 => "legacy-inventory.v2",
        }
    }
    /// Profile descriptor; neither digest grants migration/cutover authority.
    #[must_use]
    pub fn digest(self) -> Sha256Digest32 {
        match self {
            Self::V1 => legacy_inventory_profile_digest(),
            Self::LocationV2 => Sha256Digest32::from_bytes(Sha256::digest(
                b"eliot.legacy-inventory.v2\0H,I,E,T,F;12-families;7-statuses;page32;row8192;rows2000000;bytes536870912;native-limits-6;name2048;chunk65536;parents8;control134217728;objects68157440;authority-none;recovery-unresolved;physical-subject=sha256(native-locator);locator-domain=eliot-search/legacy-inventory/native-locator/v2;domain-nul;platform-u8:1=utf16be,2=unix-native,3=other-native;components-u16be:0..8;component-length-u32be:1..2048;normal-root-relative-components;kind16=failed-directory-locator;kind22=selected-profile;kind36=observed-directory-locator;kind37=locator,key,evidence;correlation-only"
            ).into()),
        }
    }
}
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
    pub const ALL: [Self; LEGACY_INVENTORY_FAMILIES] = [
        Self::Owner,
        Self::Sources,
        Self::Roots,
        Self::DirectoryManifests,
        Self::Revisions,
        Self::Preparation,
        Self::Residue,
        Self::Operations,
        Self::Publications,
        Self::Security,
        Self::Credentials,
        Self::Limits,
    ];
    /// Canonical family tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Sources => "sources",
            Self::Roots => "roots",
            Self::DirectoryManifests => "directory-manifests",
            Self::Revisions => "revisions",
            Self::Preparation => "preparation",
            Self::Residue => "residue",
            Self::Operations => "operations",
            Self::Publications => "publications",
            Self::Security => "security",
            Self::Credentials => "credentials",
            Self::Limits => "limits",
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
        match self {
            Self::Present => "present",
            Self::AbsentProven => "absent-proven",
            Self::Unsupported => "unsupported",
            Self::Corrupt => "corrupt",
            Self::Unreadable => "unreadable",
            Self::Orphan => "orphan",
            Self::Gap => "gap",
        }
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
    /// Exact admitted argument, input, encoding, I/O, native-call and open ceilings.
    /// This observation neither constructs nor renews the original allowance.
    pub native_limits: [u64; 6],
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
    pub const BASELINE: Self = Self {
        records: 2_000_000,
        encoded_bytes: 512 * 1024 * 1024,
        page_names: 32,
    };
    /// Rejects zero, inflated and footer-insufficient limits.
    /// # Errors
    /// Returns `Limit` when a component ceiling is invalid.
    pub const fn validate(self) -> Result<Self, LegacyInventoryError> {
        if self.records < 14
            || self.records > Self::BASELINE.records
            || self.encoded_bytes == 0
            || self.encoded_bytes > Self::BASELINE.encoded_bytes
            || self.page_names == 0
            || self.page_names > Self::BASELINE.page_names
        {
            Err(LegacyInventoryError::Limit)
        } else {
            Ok(self)
        }
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

/// Remaining item capacity after a non-mutating, conservative output preflight.
/// This is accounting metadata, not a reservation or migration authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyInventoryCapacity {
    /// Additional item rows available after the supplied pending rows and tail.
    pub records: u64,
    /// Additional encoded bytes available after the supplied pending bytes and tail.
    pub encoded_bytes: u64,
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
        f.write_str(match self {
            Self::Limit => "LEGACY_INVENTORY_LIMIT",
            Self::Accounting => "LEGACY_INVENTORY_ACCOUNTING",
            Self::Metadata => "LEGACY_INVENTORY_METADATA",
        })
    }
}
impl std::error::Error for LegacyInventoryError {}

/// One bounded encoder. Each returned row must be streamed before the next call.
pub struct LegacyInventoryEncoder {
    profile: LegacyInventoryProfile,
    limits: LegacyInventoryLimits,
    summary: LegacyInventorySummary,
    family: usize,
    started: bool,
    poisoned: bool,
}

impl LegacyInventoryEncoder {
    /// Starts fixed-size accounting under validated component limits.
    /// # Errors
    /// Returns `Limit` for rejected component limits.
    pub fn new(limits: LegacyInventoryLimits) -> Result<Self, LegacyInventoryError> {
        Self::with_profile(limits, LegacyInventoryProfile::V1)
    }
    /// Starts one explicitly selected profile without changing retained V1 bytes.
    /// # Errors
    /// Returns Limit for invalid component ceilings.
    pub fn with_profile(limits: LegacyInventoryLimits, profile: LegacyInventoryProfile) -> Result<Self, LegacyInventoryError> {
        Ok(Self {
            profile,
            limits: limits.validate()?,
            summary: LegacyInventorySummary::default(),
            family: 0,
            started: false,
            poisoned: false,
        })
    }
    /// Canonical header; identities retain their declared digest algorithms.
    /// # Errors
    /// Returns `Accounting` for repeated headers or invalid owner observations,
    /// and `Limit` for exhausted output bounds.
    pub fn header(&mut self, b: LegacyInventoryBinding) -> Result<Vec<u8>, LegacyInventoryError> {
        self.guarded(|this| this.header_inner(&b))
    }
    /// Encodes bounded content-free references and measured counts. Kind is numeric,
    /// with its meaning frozen by the profile, never an arbitrary input string.
    /// # Errors
    /// Returns `Accounting` for an out-of-order family and `Limit` for exhaustion.
    pub fn item(
        &mut self,
        family: LegacyInventoryFamily,
        status: LegacyInventoryStatus,
        kind: u16,
        refs: [Sha256Digest32; 3],
        counts: [u64; 3],
    ) -> Result<Vec<u8>, LegacyInventoryError> {
        self.guarded(|this| this.item_inner(family, status, kind, refs, counts))
    }

    /// Allocation-free exact size of an item, using the encoding formatter.
    /// Digest values cannot change the size of their three fixed-width fields.
    /// # Errors
    /// Returns `Limit` if the encoded row exceeds its existing bound.
    pub fn item_encoded_len(
        family: LegacyInventoryFamily,
        status: LegacyInventoryStatus,
        kind: u16,
        counts: [u64; 3],
    ) -> Result<usize, LegacyInventoryError> {
        let mut size = EncodedLength::default();
        write_item(&mut size, family, status, kind,
            [Sha256Digest32::from_bytes([0; 32]); 3], counts)
            .map_err(|_| LegacyInventoryError::Limit)?;
        if size.0 > super::MAX_SOURCE_IMPORT_ROW_BYTES {
            return Err(LegacyInventoryError::Limit);
        }
        Ok(size.0)
    }

    /// Preflights current emitted bytes/rows plus pending items and the mandatory tail.
    /// Include every future projection, fanout, synthetic gap and other item in
    /// `pending_*`; do not include terminals/footer, which this owner reserves.
    /// Unknown future item sizes need caller-supplied upper bounds from
    /// `item_encoded_len`. Terminal counters/statuses and footer numbers use
    /// conservative upper bounds, so refusal need not mean exact output cannot fit.
    /// No state is changed, poisoned or reserved; actual emission still checks limits.
    /// Recompute pending inputs against this prefix if other rows have since emitted.
    /// # Errors
    /// Returns `Accounting` before a header or after poisoning, and `Limit` on
    /// overflow or insufficient records/bytes including the remaining tail.
    pub fn preflight_remaining(
        &self,
        pending_records: u64,
        pending_encoded_bytes: u64,
    ) -> Result<LegacyInventoryCapacity, LegacyInventoryError> {
        if !self.started || self.poisoned {
            return Err(LegacyInventoryError::Accounting);
        }
        let families = LEGACY_INVENTORY_FAMILIES.checked_sub(self.family)
            .ok_or(LegacyInventoryError::Accounting)?;
        let tail_records = u64::try_from(families).map_err(|_| LegacyInventoryError::Limit)? + 1;
        let longest_status = STATUS_PRIORITY.into_iter()
            .max_by_key(|status| status.tag().len()).unwrap_or(LegacyInventoryStatus::AbsentProven);
        let mut tail = EncodedLength::default();
        for family in LegacyInventoryFamily::ALL.into_iter().skip(self.family) {
            write_terminal(&mut tail, family, longest_status, false, [self.limits.records; 7])
                .map_err(|_| LegacyInventoryError::Limit)?;
        }
        write_footer(&mut tail, self.profile, self.limits.records, self.limits.encoded_bytes)
            .map_err(|_| LegacyInventoryError::Limit)?;
        let records = self.summary.records.checked_add(pending_records)
            .and_then(|value| value.checked_add(tail_records))
            .ok_or(LegacyInventoryError::Limit)?;
        let bytes = self.summary.encoded_bytes.checked_add(pending_encoded_bytes)
            .and_then(|value| value.checked_add(u64::try_from(tail.0).ok()?))
            .ok_or(LegacyInventoryError::Limit)?;
        Ok(LegacyInventoryCapacity {
            records: self.limits.records.checked_sub(records).ok_or(LegacyInventoryError::Limit)?,
            encoded_bytes: self.limits.encoded_bytes.checked_sub(bytes).ok_or(LegacyInventoryError::Limit)?,
        })
    }
    /// Reuses the accepted content-free mapping-row encoder for every source event.
    /// # Errors
    /// Returns `Metadata` for a rejected source row, `Accounting` for wrong order
    /// or `Limit` when encoded bounds are exhausted.
    pub fn source_event(
        &mut self,
        source: &SourceImportRow,
    ) -> Result<Vec<u8>, LegacyInventoryError> {
        self.guarded(|this| this.source_event_inner(source))
    }
    /// Ends exactly the next family. Proven absence requires a frozen observed
    /// denominator; pagination alone cannot supply this flag.
    /// # Errors
    /// Returns `Accounting` for missing/out-of-order terminals or false absence,
    /// and `Limit` when output bounds are exhausted.
    pub fn end_family(
        &mut self,
        family: LegacyInventoryFamily,
        terminal: LegacyInventoryStatus,
        denominator_complete: bool,
    ) -> Result<Vec<u8>, LegacyInventoryError> {
        self.guarded(|this| this.end_family_inner(family, terminal, denominator_complete))
    }
    /// A footer exists only after all twelve terminals. It proves accounting only.
    /// # Errors
    /// Returns `Accounting` for incomplete families and `Limit` for exhaustion.
    pub fn finish(mut self) -> Result<(Vec<u8>, LegacyInventorySummary), LegacyInventoryError> {
        if self.poisoned || self.family != LEGACY_INVENTORY_FAMILIES {
            self.poisoned = true;
            return Err(LegacyInventoryError::Accounting);
        }
        let mut row = String::new();
        write_footer(&mut row, self.profile, self.summary.records, self.summary.encoded_bytes)
            .map_err(|_| LegacyInventoryError::Limit)?;
        let row = self.row(row)?;
        Ok((row, self.summary))
    }

    fn header_inner(
        &mut self,
        b: &LegacyInventoryBinding,
    ) -> Result<Vec<u8>, LegacyInventoryError> {
        if self.started || b.owner.epoch == 0 || b.owner.native_limits.contains(&0) {
            return Err(LegacyInventoryError::Accounting);
        }
        let o = b.owner;
        let registration = b.registration.map_or_else(
            || "absent".to_owned(),
            |(d, n)| format!("sha256:{}:{n}", hex(d.as_bytes())),
        );
        let row = format!(
            "H\t{}\t{}\t{}\t{}\t{}\tblake3:{}\tblake3:{}\tblake3:{}\t{}\t{}\t{}\tsha256:{}\tsha256:{}\t{registration}\t{}\t{}\t{}\tnative-limits={},{},{},{},{},{}\tauthority=none\n",
            self.profile.name(),
            hex(&o.installation),
            hex(&o.incarnation),
            hex(&o.root),
            hex(&o.invocation),
            hex(&o.request_blake3),
            hex(&o.record_blake3),
            hex(&o.physical_blake3),
            o.epoch,
            o.generation,
            b.target,
            hex(b.namespace.as_bytes()),
            hex(b.snapshot.as_bytes()),
            self.limits.records,
            self.limits.encoded_bytes,
            self.limits.page_names,
            o.native_limits[0], o.native_limits[1], o.native_limits[2],
            o.native_limits[3], o.native_limits[4], o.native_limits[5]
        );
        let bytes = self.row(row)?;
        self.started = true;
        Ok(bytes)
    }

    fn item_inner(
        &mut self,
        family: LegacyInventoryFamily,
        status: LegacyInventoryStatus,
        kind: u16,
        refs: [Sha256Digest32; 3],
        counts: [u64; 3],
    ) -> Result<Vec<u8>, LegacyInventoryError> {
        let next_count = self.next_count(family, status)?;
        let mut row = String::new();
        write_item(&mut row, family, status, kind, refs, counts)
            .map_err(|_| LegacyInventoryError::Limit)?;
        let bytes = self.row(row)?;
        self.summary.counts[family as usize][status as usize] = next_count;
        Ok(bytes)
    }

    fn source_event_inner(
        &mut self,
        source: &SourceImportRow,
    ) -> Result<Vec<u8>, LegacyInventoryError> {
        let encoded = source
            .encode()
            .map_err(|_| LegacyInventoryError::Metadata)?;
        let next_count = self.next_count(
            LegacyInventoryFamily::Sources,
            LegacyInventoryStatus::Present,
        )?;
        let bytes = self.row(format!("E\tsources\tpresent\t{}\n", hex(&encoded)))?;
        self.summary.counts[LegacyInventoryFamily::Sources as usize]
            [LegacyInventoryStatus::Present as usize] = next_count;
        Ok(bytes)
    }

    fn end_family_inner(
        &mut self,
        family: LegacyInventoryFamily,
        terminal: LegacyInventoryStatus,
        denominator_complete: bool,
    ) -> Result<Vec<u8>, LegacyInventoryError> {
        if !self.started || self.family != family as usize {
            return Err(LegacyInventoryError::Accounting);
        }
        let counts = self.summary.counts[self.family];
        let incomplete = counts[LegacyInventoryStatus::Gap as usize] > 0
            || counts[LegacyInventoryStatus::Unsupported as usize] > 0
            || counts[LegacyInventoryStatus::Unreadable as usize] > 0;
        if terminal != dominant_status(&counts) || denominator_complete == incomplete {
            return Err(LegacyInventoryError::Accounting);
        }
        let mut row = String::new();
        write_terminal(&mut row, family, terminal, denominator_complete, counts)
            .map_err(|_| LegacyInventoryError::Limit)?;
        let bytes = self.row(row)?;
        self.summary.denominators[self.family] = denominator_complete;
        self.family += 1;
        Ok(bytes)
    }

    fn next_count(
        &self,
        family: LegacyInventoryFamily,
        status: LegacyInventoryStatus,
    ) -> Result<u64, LegacyInventoryError> {
        if !self.started || self.family != family as usize {
            return Err(LegacyInventoryError::Accounting);
        }
        self.summary.counts[self.family][status as usize]
            .checked_add(1)
            .ok_or(LegacyInventoryError::Limit)
    }

    fn guarded<T>(
        &mut self,
        action: impl FnOnce(&mut Self) -> Result<T, LegacyInventoryError>,
    ) -> Result<T, LegacyInventoryError> {
        if self.poisoned {
            return Err(LegacyInventoryError::Accounting);
        }
        match action(self) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    fn row(&mut self, row: String) -> Result<Vec<u8>, LegacyInventoryError> {
        if self.poisoned {
            return Err(LegacyInventoryError::Accounting);
        }
        if row.len() > super::MAX_SOURCE_IMPORT_ROW_BYTES {
            return self.poison(LegacyInventoryError::Limit);
        }
        let Some(records) = self.summary.records.checked_add(1) else {
            return self.poison(LegacyInventoryError::Limit);
        };
        let Some(encoded_bytes) = self.summary.encoded_bytes.checked_add(row.len() as u64) else {
            return self.poison(LegacyInventoryError::Limit);
        };
        if records > self.limits.records || encoded_bytes > self.limits.encoded_bytes {
            return self.poison(LegacyInventoryError::Limit);
        }
        self.summary.records = records;
        self.summary.encoded_bytes = encoded_bytes;
        Ok(row.into_bytes())
    }

    const fn poison<T>(&mut self, error: LegacyInventoryError) -> Result<T, LegacyInventoryError> {
        self.poisoned = true;
        Err(error)
    }
}

const STATUS_PRIORITY: [LegacyInventoryStatus; 7] = [
        LegacyInventoryStatus::Gap,
        LegacyInventoryStatus::Unreadable,
        LegacyInventoryStatus::Unsupported,
        LegacyInventoryStatus::Corrupt,
        LegacyInventoryStatus::Orphan,
        LegacyInventoryStatus::Present,
        LegacyInventoryStatus::AbsentProven,
];

fn dominant_status(counts: &[u64; 7]) -> LegacyInventoryStatus {
    STATUS_PRIORITY.into_iter()
    .find(|status| counts[*status as usize] > 0)
    .unwrap_or(LegacyInventoryStatus::AbsentProven)
}

#[derive(Default)]
struct EncodedLength(usize);

impl fmt::Write for EncodedLength {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

struct Hex<'a>(&'a [u8]);
impl fmt::Display for Hex<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 { write!(output, "{byte:02x}")?; }
        Ok(())
    }
}

fn write_item(output: &mut impl fmt::Write, family: LegacyInventoryFamily,
    status: LegacyInventoryStatus, kind: u16, refs: [Sha256Digest32; 3], counts: [u64; 3],
) -> fmt::Result {
    writeln!(output, "I\t{}\t{}\t{kind}\t{}\t{}\t{}\t{}\t{}\t{}",
        family.tag(), status.tag(), Hex(refs[0].as_bytes()), Hex(refs[1].as_bytes()),
        Hex(refs[2].as_bytes()), counts[0], counts[1], counts[2])
}

fn write_terminal(output: &mut impl fmt::Write, family: LegacyInventoryFamily,
    terminal: LegacyInventoryStatus, complete: bool, counts: [u64; 7],
) -> fmt::Result {
    writeln!(output, "T\t{}\t{}\t{}\t{},{},{},{},{},{},{}",
        family.tag(), terminal.tag(), u8::from(complete), counts[0], counts[1],
        counts[2], counts[3], counts[4], counts[5], counts[6])
}

fn write_footer(output: &mut impl fmt::Write, profile: LegacyInventoryProfile,
    records: u64, bytes: u64,
) -> fmt::Result {
    writeln!(output, "F\t{}\t12\t{records}\t{bytes}\trecovery=unresolved\tcutover=none", profile.name())
}

/// Exact new profile descriptor digest; existing chain/row domains are untouched.
#[must_use]
pub fn legacy_inventory_profile_digest() -> Sha256Digest32 {
    Sha256Digest32::from_bytes(Sha256::digest(b"eliot.legacy-inventory.v1\0H,I,E,T,F;12-families;7-statuses;page32;row8192;rows2000000;bytes536870912;native-limits-6;name2048;chunk65536;parents8;control134217728;objects68157440;authority-none;recovery-unresolved").into())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 15)]));
    }
    text
}

#[cfg(test)]
mod tests;

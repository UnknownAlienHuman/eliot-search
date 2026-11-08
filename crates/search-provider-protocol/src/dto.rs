//! Closed wire values shared by the actual CLI and daemon.

use serde::{Deserialize, Deserializer, Serialize, de};
use std::fmt;

use crate::{
    MAX_ITEMS, MAX_QUERY_BYTES, MAX_REMAINING_MS, MAX_UNIT_TEXT_BYTES, ProtocolError,
    QUERY_REQUEST_ID,
};

/// One user query; the client supplies correlation and remaining time.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    /// Exact user UTF-8 query.
    pub query: String,
    /// Maximum nominations, in backend order.
    pub limit: u8,
}

impl SearchRequest {
    /// Checks actual backend input bounds.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.query.is_empty()
            || self.query.len() > MAX_QUERY_BYTES
            || self.query.as_bytes().contains(&0)
            || self.limit == 0
            || usize::from(self.limit) > MAX_ITEMS
        {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }
}

impl fmt::Debug for SearchRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchRequest")
            .field("query_bytes", &self.query.len())
            .field("limit", &self.limit)
            .finish()
    }
}

/// Literal byte search over selected sources in one committed inventory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExactRequest {
    /// UTF-8 argument bytes used literally, without decoding the retained input.
    pub literal: String,
    /// None selects all published sources; Some(empty) selects none.
    pub source_ids: Option<Vec<String>>,
    /// Maximum whole match/gap items returned, independent of scan completeness.
    pub limit: u8,
}

impl RawExactRequest {
    /// Validates the literal, canonical selection and finite output bound.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.literal.is_empty()
            || self.literal.len() > MAX_QUERY_BYTES
            || self.limit == 0
            || usize::from(self.limit) > MAX_ITEMS
        {
            return Err(ProtocolError::InvalidMessage);
        }
        if let Some(ids) = &self.source_ids {
            for (index, id) in ids.iter().enumerate() {
                if !canonical_uuid(id) || ids[..index].contains(id) {
                    return Err(ProtocolError::InvalidMessage);
                }
            }
        }
        Ok(())
    }
}

/// Literal search in a saved unchanged UTF-8 representation, never live decoding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedUtf8Request {
    /// Exact UTF-8 literal, without normalization or case folding.
    pub literal: String,
    /// None selects all committed sources; Some(empty) selects none.
    pub source_ids: Option<Vec<String>>,
    /// Maximum whole emitted items, independent of complete input scanning.
    pub limit: u8,
}

impl DecodedUtf8Request {
    /// Validates literal bytes, source selection syntax and finite output bounds.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.literal.is_empty()
            || self.literal.len() > MAX_QUERY_BYTES
            || self.limit == 0
            || usize::from(self.limit) > MAX_ITEMS
        {
            return Err(ProtocolError::InvalidMessage);
        }
        if let Some(ids) = &self.source_ids {
            for (index, id) in ids.iter().enumerate() {
                if !canonical_uuid(id) || ids[..index].contains(id) {
                    return Err(ProtocolError::InvalidMessage);
                }
            }
        }
        Ok(())
    }
}

/// The only parent-to-child messages for one query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientFrame {
    /// Start the sole query.
    Query {
        /// Exact correlation identity, currently one.
        request_id: u64,
        /// Remaining operation milliseconds sampled at send.
        remaining_ms: u64,
        /// The single shared query DTO.
        request: SearchRequest,
        /// Include bounded published/control corpus facts in a successful terminal.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        include_summary: bool,
        /// Project saved published locators onto emitted items; no live-path check.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        include_source_info: bool,
    },
    /// Start the sole raw literal execution on the same serviced connection.
    RawExact {
        /// Exact correlation identity, currently one.
        request_id: u64,
        /// Remaining original operation milliseconds sampled at send.
        remaining_ms: u64,
        /// Literal and pinned-source selection.
        request: RawExactRequest,
        /// Include bounded published/control corpus facts in a successful terminal.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        include_summary: bool,
        /// Include saved locator annotations, independent of retained-byte completeness.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        include_source_info: bool,
    },
    /// Search complete retained UTF-8 text on the same serviced connection.
    DecodedUtf8 {
        /// Exact correlation identity, currently one.
        request_id: u64,
        /// Remaining original execution milliseconds sampled at send.
        remaining_ms: u64,
        /// Literal and selected committed source identities.
        request: DecodedUtf8Request,
        /// Include bounded published/control corpus facts in a successful terminal.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        include_summary: bool,
        /// Include saved locator annotations without reading live source paths.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        include_source_info: bool,
    },
    /// Cancel that same query without another work slot.
    Cancel {
        /// Exact active query identity.
        request_id: u64,
    },
}

impl ClientFrame {
    /// Whether each emitted item must contain a saved-source annotation.
    #[must_use]
    pub const fn include_source_info(&self) -> bool {
        match self {
            Self::Query {
                include_source_info,
                ..
            }
            | Self::RawExact {
                include_source_info,
                ..
            }
            | Self::DecodedUtf8 {
                include_source_info,
                ..
            } => *include_source_info,
            Self::Cancel { .. } => false,
        }
    }
    /// Whether this data request requires a corpus summary on success.
    #[must_use]
    pub const fn include_summary(&self) -> bool {
        match self {
            Self::Query {
                include_summary, ..
            }
            | Self::RawExact {
                include_summary, ..
            }
            | Self::DecodedUtf8 {
                include_summary, ..
            } => *include_summary,
            Self::Cancel { .. } => false,
        }
    }

    /// Validates correlation, finite duration and backend query bounds.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let request_id = match self {
            Self::Query {
                request_id,
                remaining_ms,
                request,
                ..
            } => {
                if *remaining_ms == 0 || *remaining_ms > MAX_REMAINING_MS {
                    return Err(ProtocolError::InvalidMessage);
                }
                request.validate()?;
                *request_id
            }
            Self::RawExact {
                request_id,
                remaining_ms,
                request,
                ..
            } => {
                if *remaining_ms == 0 || *remaining_ms > MAX_REMAINING_MS {
                    return Err(ProtocolError::InvalidMessage);
                }
                request.validate()?;
                *request_id
            }
            Self::DecodedUtf8 {
                request_id,
                remaining_ms,
                request,
                ..
            } => {
                if *remaining_ms == 0 || *remaining_ms > MAX_REMAINING_MS {
                    return Err(ProtocolError::InvalidMessage);
                }
                request.validate()?;
                *request_id
            }
            Self::Cancel { request_id } => *request_id,
        };
        if request_id != QUERY_REQUEST_ID {
            return Err(ProtocolError::InvalidCorrelation);
        }
        Ok(())
    }
}

/// Stored plain-object kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKindDto {
    /// Exact retained acquired bytes.
    Source,
    /// Complete retained prepared representation and unit descriptors.
    Prepared,
}

/// Integrity address of an existing retained object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectRefDto {
    /// Selected plain-object format, currently one.
    pub version: u16,
    /// Exact content kind.
    pub kind: ObjectKindDto,
    /// Lowercase hexadecimal BLAKE3 of the complete stored bytes.
    pub digest: String,
    /// Complete stored byte length.
    pub bytes: u64,
}

impl ObjectRefDto {
    fn validate(&self, expected: ObjectKindDto) -> Result<(), ProtocolError> {
        let maximum = match expected {
            ObjectKindDto::Source => 8 * 1024 * 1024,
            ObjectKindDto::Prepared => 64 * 1024 * 1024,
        };
        if self.version != 1
            || self.kind != expected
            || !lower_hex(&self.digest, 64)
            || self.bytes > maximum
            || (expected == ObjectKindDto::Prepared && self.bytes == 0)
        {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }
}

/// Immutable retained citation projected from one published snapshot.
///
/// Captured source/preparation revisions remain inside the daemon resolver.
/// This value makes no claim about an available live source path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceMetadata {
    /// Exact committed generation sequence.
    pub generation: u64,
    /// Lowercase hexadecimal generation-bound point identity.
    pub point_id: String,
    /// Canonical persisted source namespace UUID.
    pub source_namespace_id: String,
    /// Canonical persisted logical source UUID.
    pub source_id: String,
    /// Exact retained source input.
    pub input: ObjectRefDto,
    /// Exact complete prepared artifact.
    pub prepared: ObjectRefDto,
    /// Zero-based unit descriptor ordinal.
    pub unit: u64,
    /// Inclusive representation byte start.
    pub start: u64,
    /// Exclusive representation byte end.
    pub end: u64,
    /// Inclusive logical-line start.
    pub line_start: u64,
    /// Exclusive logical-line end.
    pub line_end: u64,
    /// Whether the unit begins at a complete line boundary.
    pub starts_at_line_boundary: bool,
    /// Whether the unit ends at a complete line boundary.
    pub ends_at_line_boundary: bool,
}

impl EvidenceMetadata {
    fn validate(&self, generation: u64) -> Result<(), ProtocolError> {
        if generation == 0
            || self.generation != generation
            || !lower_hex(&self.point_id, 32)
            || u64::from_str_radix(&self.point_id[..16], 16).ok() != Some(generation)
            || !canonical_uuid(&self.source_namespace_id)
            || !canonical_uuid(&self.source_id)
            || self.start >= self.end
            || self.end - self.start > MAX_UNIT_TEXT_BYTES as u64
            || self.line_start >= self.line_end
        {
            return Err(ProtocolError::InvalidMessage);
        }
        self.input.validate(ObjectKindDto::Source)?;
        if self.end > self.input.bytes {
            return Err(ProtocolError::InvalidMessage);
        }
        self.prepared.validate(ObjectKindDto::Prepared)
    }
}

/// Source-path freshness is deliberately not observed by a retained query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceLiveStatus {
    /// Saved metadata only; no source stat/open or current-file verification.
    NotChecked,
}

/// Closed reasons a valid retained item has no displayable saved locator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceInfoUnavailableReason {
    /// The saved locator cannot be decoded as a supported absolute native path.
    InvalidLocator,
    /// The current platform has no supported native locator decoder.
    UnsupportedPlatform,
    /// The saved native path has no lossless UTF-8 string representation.
    NonUnicodeLocator,
    /// The bounded annotation allocation was unavailable.
    AllocationFailed,
    /// The optional annotation did not fit the existing reply budget.
    OutputLimit,
}

/// An annotation bound by its enclosing immutable citation, never a live-file claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceInfo {
    /// Exact saved locator decoded without filesystem observation or normalization.
    Available {
        /// A navigation hint; the named file may be absent, changed or relocated.
        saved_path_hint: String,
        /// Always explicitly not checked.
        live_status: SourceLiveStatus,
    },
    /// Retained evidence remains usable independently of this metadata annotation.
    Unavailable {
        /// Closed annotation-only failure.
        reason: SourceInfoUnavailableReason,
        /// No live-file verification was attempted.
        live_status: SourceLiveStatus,
    },
}

impl SourceInfo {
    const fn validate(&self) -> Result<(), ProtocolError> {
        if let Self::Available {
            saved_path_hint, ..
        } = self
            && (saved_path_hint.is_empty() || saved_path_hint.len() > MAX_UNIT_TEXT_BYTES)
        {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }

    pub(crate) fn omit_for_output(&mut self) {
        if matches!(self, Self::Available { .. }) {
            *self = Self::Unavailable {
                reason: SourceInfoUnavailableReason::OutputLimit,
                live_status: SourceLiveStatus::NotChecked,
            };
        }
    }
}

fn source_info_requested(info: Option<&SourceInfo>, requested: bool) -> Result<(), ProtocolError> {
    if info.is_some() != requested {
        return Err(ProtocolError::InvalidMessage);
    }
    if let Some(info) = info {
        info.validate()?;
    }
    Ok(())
}

/// One complete verified prepared unit in backend rank order.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchHit {
    /// Original contiguous one-based backend rank.
    pub rank: u8,
    /// Finite backend score, without normalization or a positivity rule.
    pub score: f32,
    /// Immutable retained citation.
    pub evidence: EvidenceMetadata,
    /// Exact full UTF-8 unit; never clipped for transport.
    pub unit_text: String,
    /// Optional saved locator, tied to this item's published source binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_info: Option<SourceInfo>,
}

impl fmt::Debug for SearchHit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchHit")
            .field("rank", &self.rank)
            .field("score", &self.score)
            .field("evidence", &self.evidence)
            .field("unit_text_bytes", &self.unit_text.len())
            .field("source_info_present", &self.source_info.is_some())
            .finish()
    }
}

/// Content-free bounded failure code, serialized as a JSON string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct FailureCode(String);

impl FailureCode {
    /// Accepts 1..=64 uppercase ASCII letters, digits and underscores.
    pub fn new(code: &str) -> Result<Self, ProtocolError> {
        if code.is_empty()
            || code.len() > 64
            || !code
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(Self(code.to_owned()))
    }

    /// The validated content-free code.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for FailureCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        Self::new(&code).map_err(de::Error::custom)
    }
}

/// An unavailable retained object for a ranked, otherwise validated citation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchGap {
    /// Original contiguous one-based backend rank.
    pub rank: u8,
    /// Original finite backend score.
    pub score: f32,
    /// Citation of the missing retained object.
    pub evidence: EvidenceMetadata,
    /// `SOURCE_OBJECT_MISSING` or `PREPARED_OBJECT_MISSING`.
    pub code: FailureCode,
    /// Saved metadata may remain available even when a retained object is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_info: Option<SourceInfo>,
}

/// One whole hit or missing-object gap, preserving backend order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SearchItem {
    /// A complete verified prepared unit.
    Hit(SearchHit),
    /// An explicitly missing retained object.
    Gap(SearchGap),
}

impl SearchItem {
    /// Saved-source annotation on this emitted hit or gap.
    #[must_use]
    pub const fn source_info(&self) -> Option<&SourceInfo> {
        match self {
            Self::Hit(hit) => hit.source_info.as_ref(),
            Self::Gap(gap) => gap.source_info.as_ref(),
        }
    }
    /// Original one-based backend rank.
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Hit(hit) => hit.rank,
            Self::Gap(gap) => gap.rank,
        }
    }

    /// Immutable retained citation.
    #[must_use]
    pub const fn evidence(&self) -> &EvidenceMetadata {
        match self {
            Self::Hit(hit) => &hit.evidence,
            Self::Gap(gap) => &gap.evidence,
        }
    }

    fn validate(&self, generation: u64) -> Result<(), ProtocolError> {
        if let Some(info) = self.source_info() {
            info.validate()?;
        }
        let score = match self {
            Self::Hit(hit) => {
                if hit.unit_text.len() > MAX_UNIT_TEXT_BYTES
                    || u64::try_from(hit.unit_text.len()).ok()
                        != hit.evidence.end.checked_sub(hit.evidence.start)
                {
                    return Err(ProtocolError::InvalidMessage);
                }
                hit.score
            }
            Self::Gap(gap) => {
                if !matches!(
                    gap.code.as_str(),
                    "SOURCE_OBJECT_MISSING" | "PREPARED_OBJECT_MISSING"
                ) {
                    return Err(ProtocolError::InvalidMessage);
                }
                gap.score
            }
        };
        if !score.is_finite() {
            return Err(ProtocolError::InvalidMessage);
        }
        self.evidence().validate(generation)
    }
}

/// Whether the complete terminal carries the whole requested rank prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// All nominations returned for the requested limit were represented.
    Complete,
    /// The next whole item would exceed the complete-frame allowance.
    OutputLimit,
}

/// A source revision from the committed inventory, without a prepared-point dependency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSourceRef {
    /// Canonical persisted source namespace UUID.
    pub source_namespace_id: String,
    /// Canonical persisted source UUID.
    pub source_id: String,
    /// Exact committed source-row revision.
    pub row_revision: u64,
    /// Exact committed registration revision.
    pub registration_revision: u64,
    /// Integrity address of the retained raw bytes.
    pub input: ObjectRefDto,
}

impl RawSourceRef {
    fn validate(&self) -> Result<(), ProtocolError> {
        if !canonical_uuid(&self.source_namespace_id)
            || !canonical_uuid(&self.source_id)
            || self.row_revision == 0
            || self.registration_revision == 0
        {
            return Err(ProtocolError::InvalidMessage);
        }
        self.input.validate(ObjectKindDto::Source)
    }
}

/// One verified raw-byte literal occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExactMatch {
    /// Pinned source revision and retained object.
    pub source: RawSourceRef,
    /// Inclusive byte start in that raw object.
    pub start: u64,
    /// Exclusive byte end in that raw object.
    pub end: u64,
    /// Exact matched bytes in lowercase hex, also valid for non-UTF-8 input.
    pub bytes_hex: String,
    /// Optional saved locator for the enclosing pinned source reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_info: Option<SourceInfo>,
}

/// An explicitly missing selected raw object; preparation Gaps do not cause this gap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExactGap {
    /// Pinned source revision and missing object.
    pub source: RawSourceRef,
    /// Only `SOURCE_OBJECT_MISSING`; corrupt bytes are a failed terminal.
    pub code: FailureCode,
    /// Optional saved locator; missing retained bytes do not erase saved metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_info: Option<SourceInfo>,
}

/// One whole raw match or missing-source gap in source/byte order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawExactItem {
    /// Verified raw occurrence; overlapping ranges are allowed.
    Match(RawExactMatch),
    /// Missing retained raw input.
    Gap(RawExactGap),
}

impl RawExactItem {
    /// Saved-source annotation on this emitted item.
    #[must_use]
    pub const fn source_info(&self) -> Option<&SourceInfo> {
        match self {
            Self::Match(hit) => hit.source_info.as_ref(),
            Self::Gap(gap) => gap.source_info.as_ref(),
        }
    }

    pub(crate) fn omit_source_info_for_output(&mut self) {
        let info = match self {
            Self::Match(hit) => &mut hit.source_info,
            Self::Gap(gap) => &mut gap.source_info,
        };
        if let Some(info) = info {
            info.omit_for_output();
        }
    }
    /// Exact committed source binding for this item.
    #[must_use]
    pub const fn source(&self) -> &RawSourceRef {
        match self {
            Self::Match(hit) => &hit.source,
            Self::Gap(gap) => &gap.source,
        }
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if let Some(info) = self.source_info() {
            info.validate()?;
        }
        self.source().validate()?;
        match self {
            Self::Match(hit) => {
                let length = hit
                    .end
                    .checked_sub(hit.start)
                    .filter(|n| *n != 0)
                    .ok_or(ProtocolError::InvalidMessage)?;
                if hit.end > hit.source.input.bytes
                    || length > MAX_QUERY_BYTES as u64
                    || !lower_hex(
                        &hit.bytes_hex,
                        usize::try_from(length * 2).map_err(|_| ProtocolError::InvalidMessage)?,
                    )
                {
                    return Err(ProtocolError::InvalidMessage);
                }
            }
            Self::Gap(gap) if gap.code.as_str() != "SOURCE_OBJECT_MISSING" => {
                return Err(ProtocolError::InvalidMessage);
            }
            Self::Gap(_) => {}
        }
        Ok(())
    }
}

/// Exhaustive raw execution accounting, separate from its bounded emitted prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExactResult {
    /// Exact committed generation.
    pub generation: u64,
    /// Selected committed inventory size, including empty and preparation-Gap inputs.
    pub selected_sources: u64,
    /// Sources whose retained bytes were verified and fully scanned.
    pub scanned_sources: u64,
    /// Selected retained objects found missing.
    pub missing_sources: u64,
    /// Actual verified raw bytes scanned, including work beyond the emitted prefix.
    pub scanned_bytes: u64,
    /// Total occurrences found, including those beyond the presentation limit.
    pub matches: u64,
    /// Whole source/byte-order prefix, bounded before decoding an extra item.
    #[serde(deserialize_with = "deserialize_items")]
    pub items: Vec<RawExactItem>,
    /// Every selected retained input was verified and fully scanned; false for missing inputs.
    pub scan_complete: bool,
    /// True only for complete scanning, zero matches and no missing input.
    pub complete_negative: bool,
    /// Complete emitted items versus a whole-item presentation cut.
    pub stop: StopReason,
}

impl RawExactResult {
    /// Validates this raw result against its exact literal and selected inventory.
    ///
    /// An explicit selection retains its full denominator; emitted items must belong
    /// to it. Matched hex bytes must equal the original UTF-8 literal byte for byte.
    pub fn validate_request(&self, request: &RawExactRequest) -> Result<(), ProtocolError> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        request.validate()?;
        self.validate(request.limit)?;
        if let Some(ids) = &request.source_ids
            && (u64::try_from(ids.len()).ok() != Some(self.selected_sources)
                || self
                    .items
                    .iter()
                    .any(|item| !ids.contains(&item.source().source_id)))
        {
            return Err(ProtocolError::InvalidMessage);
        }
        let literal = request.literal.as_bytes();
        let length = u64::try_from(literal.len()).map_err(|_| ProtocolError::InvalidMessage)?;
        for item in &self.items {
            if let RawExactItem::Match(hit) = item
                && (hit.end - hit.start != length
                    || hit
                        .bytes_hex
                        .as_bytes()
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .zip(literal)
                        .any(|(pair, byte)| {
                            pair[0] != DIGITS[usize::from(*byte >> 4)]
                                || pair[1] != DIGITS[usize::from(*byte & 15)]
                        }))
            {
                return Err(ProtocolError::InvalidMessage);
            }
        }
        Ok(())
    }

    /// Checks accounting, source bindings, byte ranges and complete-negative claims.
    pub fn validate(&self, requested_limit: u8) -> Result<(), ProtocolError> {
        let total = self
            .matches
            .checked_add(self.missing_sources)
            .ok_or(ProtocolError::InvalidMessage)?;
        if self.generation == 0
            || requested_limit == 0
            || usize::from(requested_limit) > MAX_ITEMS
            || self.items.len() > usize::from(requested_limit)
            || self.scanned_sources.checked_add(self.missing_sources) != Some(self.selected_sources)
            || self.scan_complete != (self.scanned_sources == self.selected_sources)
            || self.complete_negative != (self.scan_complete && self.matches == 0)
            || self.matches > self.scanned_bytes
            || (self.scanned_sources == 0 && self.scanned_bytes != 0)
            || u64::try_from(self.items.len())
                .ok()
                .is_none_or(|count| count > total)
            || (self.stop == StopReason::Complete) != (self.items.len() as u64 == total)
        {
            return Err(ProtocolError::InvalidMessage);
        }
        let mut match_count = 0_u64;
        let mut gap_count = 0_u64;
        for (index, item) in self.items.iter().enumerate() {
            item.validate()?;
            match item {
                RawExactItem::Match(_) => match_count += 1,
                RawExactItem::Gap(_) => gap_count += 1,
            }
            if let Some(previous) = index.checked_sub(1).map(|i| &self.items[i]) {
                let left = previous.source();
                let right = item.source();
                if left.source_namespace_id != right.source_namespace_id
                    || left.source_id > right.source_id
                {
                    return Err(ProtocolError::InvalidMessage);
                }
                if left.source_id == right.source_id
                    && (left != right
                        || !matches!((previous, item),
                        (RawExactItem::Match(a), RawExactItem::Match(b)) if a.start < b.start))
                {
                    return Err(ProtocolError::InvalidMessage);
                }
            }
        }
        if match_count > self.matches || gap_count > self.missing_sources {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }
}

/// One verified literal occurrence in a retained UTF-8 representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedUtf8Match {
    /// Source revision and exact retained input.
    pub source: RawSourceRef,
    /// Verified representation bound to this source and its pinned settings.
    pub prepared: ObjectRefDto,
    /// Inclusive UTF-8 representation byte start.
    pub representation_start: u64,
    /// Exclusive UTF-8 representation byte end.
    pub representation_end: u64,
    /// Inclusive source byte start, verified through the identity map.
    pub source_start: u64,
    /// Exclusive source byte end, verified through the identity map.
    pub source_end: u64,
    /// Exact matching UTF-8 text, including original BOM or line endings.
    pub text: String,
    /// Optional saved locator, independent of representation verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_info: Option<SourceInfo>,
}

/// A selected source without an available valid retained UTF-8 representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedUtf8Gap {
    /// Source revision from the pinned committed inventory.
    pub source: RawSourceRef,
    /// Selected Ready artifact; absent only for a preparation Gap.
    pub prepared: Option<ObjectRefDto>,
    /// `PREPARATION_GAP`, `SOURCE_OBJECT_MISSING` or `PREPARED_OBJECT_MISSING`.
    pub code: FailureCode,
    /// Optional saved source metadata even when the representation is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_info: Option<SourceInfo>,
}

/// One whole UTF-8 occurrence or unavailable-representation gap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DecodedUtf8Item {
    /// Retained representation match with a verified identity source map.
    Match(DecodedUtf8Match),
    /// A preparation Gap or missing input/artifact; corruption fails.
    Gap(DecodedUtf8Gap),
}

impl DecodedUtf8Item {
    /// Saved-source annotation on this emitted item.
    #[must_use]
    pub const fn source_info(&self) -> Option<&SourceInfo> {
        match self {
            Self::Match(hit) => hit.source_info.as_ref(),
            Self::Gap(gap) => gap.source_info.as_ref(),
        }
    }

    pub(crate) fn omit_source_info_for_output(&mut self) {
        let info = match self {
            Self::Match(hit) => &mut hit.source_info,
            Self::Gap(gap) => &mut gap.source_info,
        };
        if let Some(info) = info {
            info.omit_for_output();
        }
    }
    /// Source binding in the committed inventory.
    #[must_use]
    pub const fn source(&self) -> &RawSourceRef {
        match self {
            Self::Match(hit) => &hit.source,
            Self::Gap(gap) => &gap.source,
        }
    }

    fn validate(&self) -> Result<(), ProtocolError> {
        if let Some(info) = self.source_info() {
            info.validate()?;
        }
        self.source().validate()?;
        match self {
            Self::Match(hit) => {
                hit.prepared.validate(ObjectKindDto::Prepared)?;
                if hit.representation_start >= hit.representation_end
                    || hit.source_start != hit.representation_start
                    || hit.source_end != hit.representation_end
                    || hit.source_end > hit.source.input.bytes
                    || hit.text.len() > MAX_QUERY_BYTES
                    || u64::try_from(hit.text.len()).ok()
                        != hit.source_end.checked_sub(hit.source_start)
                {
                    return Err(ProtocolError::InvalidMessage);
                }
            }
            Self::Gap(gap) => match (gap.code.as_str(), &gap.prepared) {
                ("PREPARATION_GAP", None) => {}
                ("SOURCE_OBJECT_MISSING" | "PREPARED_OBJECT_MISSING", Some(prepared)) => {
                    prepared.validate(ObjectKindDto::Prepared)?;
                }
                _ => return Err(ProtocolError::InvalidMessage),
            },
        }
        Ok(())
    }
}

/// Complete retained UTF-8 accounting, separate from its emitted prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedUtf8Result {
    /// Exact committed generation.
    pub generation: u64,
    /// Selected sources, including empty and preparation-Gap inputs.
    pub selected_sources: u64,
    /// Fully verified and scanned retained representations.
    pub scanned_sources: u64,
    /// Selected sources without an available valid retained representation.
    pub gap_sources: u64,
    /// Actual verified representation bytes scanned.
    pub scanned_bytes: u64,
    /// Total occurrences, including those beyond the emitted prefix.
    pub matches: u64,
    /// Source/range ordered whole-item prefix, bounded during decoding.
    #[serde(deserialize_with = "deserialize_items")]
    pub items: Vec<DecodedUtf8Item>,
    /// Every selected source had a verified fully scanned representation.
    pub scan_complete: bool,
    /// Complete scanning, zero matches and no representation gap.
    pub complete_negative: bool,
    /// Emitted prefix completeness, independent of scanning completeness.
    pub stop: StopReason,
}

impl DecodedUtf8Result {
    /// Checks the literal and the entire explicit selection denominator.
    pub fn validate_request(&self, request: &DecodedUtf8Request) -> Result<(), ProtocolError> {
        request.validate()?;
        self.validate(request.limit)?;
        if let Some(ids) = &request.source_ids
            && (u64::try_from(ids.len()).ok() != Some(self.selected_sources)
                || self
                    .items
                    .iter()
                    .any(|item| !ids.contains(&item.source().source_id)))
        {
            return Err(ProtocolError::InvalidMessage);
        }
        for item in &self.items {
            if let DecodedUtf8Item::Match(hit) = item
                && hit.text != request.literal
            {
                return Err(ProtocolError::InvalidMessage);
            }
        }
        Ok(())
    }

    /// Checks source/Prepared bindings, identity ranges, order and accounting.
    pub fn validate(&self, requested_limit: u8) -> Result<(), ProtocolError> {
        let total = self
            .matches
            .checked_add(self.gap_sources)
            .ok_or(ProtocolError::InvalidMessage)?;
        if self.generation == 0
            || requested_limit == 0
            || usize::from(requested_limit) > MAX_ITEMS
            || self.items.len() > usize::from(requested_limit)
            || self.scanned_sources.checked_add(self.gap_sources) != Some(self.selected_sources)
            || self.scan_complete != (self.scanned_sources == self.selected_sources)
            || self.complete_negative != (self.scan_complete && self.matches == 0)
            || self.matches > self.scanned_bytes
            || (self.scanned_sources == 0 && self.scanned_bytes != 0)
            || self.items.len() as u64 > total
            || (self.stop == StopReason::Complete) != (self.items.len() as u64 == total)
        {
            return Err(ProtocolError::InvalidMessage);
        }
        let mut matches = 0_u64;
        let mut gaps = 0_u64;
        for (index, item) in self.items.iter().enumerate() {
            item.validate()?;
            match item {
                DecodedUtf8Item::Match(_) => matches += 1,
                DecodedUtf8Item::Gap(_) => gaps += 1,
            }
            if let Some(previous) = index.checked_sub(1).map(|i| &self.items[i]) {
                let left = previous.source();
                let right = item.source();
                if left.source_namespace_id != right.source_namespace_id
                    || left.source_id > right.source_id
                    || (left.source_id == right.source_id
                        && (left != right
                            || !matches!((previous, item),
                            (DecodedUtf8Item::Match(a), DecodedUtf8Item::Match(b))
                            if a.prepared == b.prepared && a.representation_start < b.representation_start)))
                {
                    return Err(ProtocolError::InvalidMessage);
                }
            }
        }
        if matches > self.matches || gaps > self.gap_sources {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }
}

/// Exactly one application terminal, with typed failures distinct from transport loss.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum QueryOutcome {
    /// Complete terminal containing whole items only.
    Complete {
        /// Exact committed generation sequence.
        generation: u64,
        /// Original rank prefix, bounded before deserializing a 65th item.
        items: Vec<SearchItem>,
        /// Explicit output completeness.
        stop: StopReason,
    },
    /// Raw execution completed with explicit scan and presentation completeness.
    RawExact {
        /// Pinned source-based result, independent of prepared points.
        result: RawExactResult,
    },
    /// Completed search in saved unchanged UTF-8 representations.
    DecodedUtf8 {
        /// Representation-bound matches, gaps and completeness.
        result: DecodedUtf8Result,
    },
    /// The request was cancelled before terminal output committed.
    Cancelled,
    /// An application failure; corruption never becomes an empty result.
    Failed {
        /// Validated content-free code.
        code: FailureCode,
    },
}

impl<'de> Deserialize<'de> for QueryOutcome {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Serde's internally tagged unit variant accepts extra map fields.
        // A closed empty struct preserves the public unit variant and rejects
        // hidden data on a cancelled terminal.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct EmptyOutcome {}
        #[derive(Deserialize)]
        #[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
        enum DecodedOutcome {
            Complete {
                generation: u64,
                #[serde(deserialize_with = "deserialize_items")]
                items: Vec<SearchItem>,
                stop: StopReason,
            },
            RawExact {
                result: RawExactResult,
            },
            DecodedUtf8 {
                result: DecodedUtf8Result,
            },
            Cancelled(EmptyOutcome),
            Failed {
                code: FailureCode,
            },
        }
        match DecodedOutcome::deserialize(deserializer)? {
            DecodedOutcome::Complete {
                generation,
                items,
                stop,
            } => Ok(Self::Complete {
                generation,
                items,
                stop,
            }),
            DecodedOutcome::RawExact { result } => Ok(Self::RawExact { result }),
            DecodedOutcome::DecodedUtf8 { result } => Ok(Self::DecodedUtf8 { result }),
            DecodedOutcome::Cancelled(_) => Ok(Self::Cancelled),
            DecodedOutcome::Failed { code } => Ok(Self::Failed { code }),
        }
    }
}

/// Why current CONTROL metadata could not be observed for a valid retained result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SummaryUnavailableReason {
    /// Current metadata could not be read.
    Unavailable,
    /// Current metadata uses an unsupported source schema.
    UnsupportedSchema,
    /// Current metadata is malformed or contradicts the published revision.
    Corrupt,
}

/// A bounded current-CONTROL observation, never a live-filesystem freshness claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlObservation {
    /// Current metadata was read in the pinned snapshot.
    Known {
        /// Corpus-wide wanted revision observed in CONTROL.
        wanted_revision: u64,
        /// True exactly when current CONTROL is newer than the published revision.
        known_control_lag: bool,
    },
    /// Published counts remain known while current metadata is unavailable.
    Unavailable {
        /// Closed metadata-only cause; no numeric freshness value is invented.
        reason: SummaryUnavailableReason,
    },
}

/// Published and selected corpus facts independent of emitted cards and Raw completeness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuerySummary {
    /// Wanted revision captured by the pinned published generation.
    pub published_wanted_revision: u64,
    /// Total sources in that committed inventory, including Empty and Gap outcomes.
    pub published_sources: u64,
    /// Committed preparation Gap outcomes in the entire published inventory.
    pub published_preparation_gaps: u64,
    /// Sources in the original query's selected published scope.
    pub selected_sources: u64,
    /// Committed preparation Gap outcomes in that selected scope.
    pub selected_preparation_gaps: u64,
    /// Known current revision/lag, or a typed current-metadata observation failure.
    pub control: ControlObservation,
}

impl QuerySummary {
    /// Checks existing source bounds and corpus-wide CONTROL lag consistency.
    pub const fn validate(&self) -> Result<(), ProtocolError> {
        if self.published_sources > 4096
            || self.published_preparation_gaps > self.published_sources
            || self.selected_sources > self.published_sources
            || self.selected_preparation_gaps > self.selected_sources
            || self.selected_preparation_gaps > self.published_preparation_gaps
        {
            return Err(ProtocolError::InvalidMessage);
        }
        if self.selected_sources - self.selected_preparation_gaps
            > self.published_sources - self.published_preparation_gaps
        {
            return Err(ProtocolError::InvalidMessage);
        }
        if let ControlObservation::Known {
            wanted_revision,
            known_control_lag,
        } = &self.control
            && (*wanted_revision < self.published_wanted_revision
                || *known_control_lag != (*wanted_revision > self.published_wanted_revision))
        {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }

    fn validate_selection(&self, source_ids: Option<&[String]>) -> Result<(), ProtocolError> {
        self.validate()?;
        let selected = if let Some(ids) = source_ids {
            u64::try_from(ids.len()).map_err(|_| ProtocolError::InvalidMessage)?
        } else {
            if self.selected_preparation_gaps != self.published_preparation_gaps {
                return Err(ProtocolError::InvalidMessage);
            }
            self.published_sources
        };
        if self.selected_sources != selected {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }
}

/// The sole child-to-parent terminal for request one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    /// Exact query correlation identity.
    pub request_id: u64,
    /// Complete application outcome.
    pub outcome: QueryOutcome,
    /// Requested corpus facts; absent on old-wire or failed/cancelled terminals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<QuerySummary>,
}

impl Terminal {
    /// Validates complete correlation, bounds, citation shape and ordered ranks.
    pub fn validate(&self, requested_limit: u8) -> Result<(), ProtocolError> {
        if self.request_id != QUERY_REQUEST_ID {
            return Err(ProtocolError::InvalidCorrelation);
        }
        if requested_limit == 0 || usize::from(requested_limit) > MAX_ITEMS {
            return Err(ProtocolError::InvalidMessage);
        }
        if let QueryOutcome::Complete {
            generation, items, ..
        } = &self.outcome
        {
            if *generation == 0 || items.len() > usize::from(requested_limit) {
                return Err(ProtocolError::InvalidMessage);
            }
            for (index, item) in items.iter().enumerate() {
                if usize::from(item.rank()) != index + 1
                    || items[..index]
                        .iter()
                        .any(|prior| prior.evidence().point_id == item.evidence().point_id)
                {
                    return Err(ProtocolError::InvalidMessage);
                }
                item.validate(*generation)?;
            }
        }
        if let QueryOutcome::RawExact { result } = &self.outcome {
            result.validate(requested_limit)?;
        }
        if let QueryOutcome::DecodedUtf8 { result } = &self.outcome {
            result.validate(requested_limit)?;
        }
        if let Some(summary) = &self.summary {
            summary.validate()?;
            let selected = match &self.outcome {
                QueryOutcome::Complete { .. } => {
                    summary.validate_selection(None)?;
                    summary.selected_sources
                }
                QueryOutcome::RawExact { result } => result.selected_sources,
                QueryOutcome::DecodedUtf8 { result } => result.selected_sources,
                QueryOutcome::Cancelled | QueryOutcome::Failed { .. } => {
                    return Err(ProtocolError::InvalidMessage);
                }
            };
            if summary.selected_sources != selected {
                return Err(ProtocolError::InvalidMessage);
            }
        }
        Ok(())
    }

    /// Binds outcome, summary presence and selected scope to the original data request.
    pub fn validate_request(&self, request: &ClientFrame) -> Result<(), ProtocolError> {
        request.validate()?;
        let limit = match request {
            ClientFrame::Query { request, .. } => request.limit,
            ClientFrame::RawExact { request, .. } => request.limit,
            ClientFrame::DecodedUtf8 { request, .. } => request.limit,
            ClientFrame::Cancel { .. } => return Err(ProtocolError::InvalidMessage),
        };
        self.validate(limit)?;
        let selection = match (request, &self.outcome) {
            (ClientFrame::Query { .. }, QueryOutcome::Complete { .. }) => None,
            (ClientFrame::RawExact { request, .. }, QueryOutcome::RawExact { result }) => {
                result.validate_request(request)?;
                request.source_ids.as_deref()
            }
            (ClientFrame::DecodedUtf8 { request, .. }, QueryOutcome::DecodedUtf8 { result }) => {
                result.validate_request(request)?;
                request.source_ids.as_deref()
            }
            (_, QueryOutcome::Cancelled | QueryOutcome::Failed { .. }) => return Ok(()),
            _ => return Err(ProtocolError::InvalidMessage),
        };
        if self.summary.is_some() != request.include_summary() {
            return Err(ProtocolError::InvalidMessage);
        }
        if let Some(summary) = &self.summary {
            summary.validate_selection(selection)?;
        }
        let requested = request.include_source_info();
        match &self.outcome {
            QueryOutcome::Complete { items, .. } => {
                for item in items {
                    source_info_requested(item.source_info(), requested)?;
                }
            }
            QueryOutcome::RawExact { result } => {
                for item in &result.items {
                    source_info_requested(item.source_info(), requested)?;
                }
            }
            QueryOutcome::DecodedUtf8 { result } => {
                for item in &result.items {
                    source_info_requested(item.source_info(), requested)?;
                }
            }
            QueryOutcome::Cancelled | QueryOutcome::Failed { .. } => {}
        }
        Ok(())
    }
}

/// The original query executed once when opening this child's sole window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "mode",
    content = "request",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum WindowQuery {
    /// Ranked retrieval in original backend order.
    Search(SearchRequest),
    /// Raw retained-byte literal scan in source/range order.
    Raw(RawExactRequest),
    /// Saved unchanged UTF-8 literal scan in source/range order.
    DecodedUtf8(DecodedUtf8Request),
}

impl WindowQuery {
    /// Checks the actual query's existing limits and selection semantics.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Search(request) => request.validate(),
            Self::Raw(request) => request.validate(),
            Self::DecodedUtf8(request) => request.validate(),
        }
    }

    /// Maximum size of the original retained result prefix, never all Exact matches.
    #[must_use]
    pub const fn limit(&self) -> u8 {
        match self {
            Self::Search(request) => request.limit,
            Self::Raw(request) => request.limit,
            Self::DecodedUtf8(request) => request.limit,
        }
    }
}

/// One session data/control frame; request identities are scoped to this owned child.
/// Open is accepted once. Later data identities increase; Cancel names active work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionClientFrame {
    /// Execute the original query and return its first bounded page.
    Open {
        /// Nonzero request correlation identity.
        request_id: u64,
        /// Original operation milliseconds remaining, sampled at send.
        remaining_ms: u64,
        /// Query kind and original bounded result limit.
        query: WindowQuery,
        /// First-page size, 1..=64; omitted JSON uses 16.
        #[serde(default = "default_window_page_size")]
        page_size: u8,
        /// Fixed lifetime from accepted Open before work or pinning; never renewed.
        #[serde(default = "default_window_ttl")]
        ttl_ms: u64,
        /// Cache corpus facts from Open for the original window lifetime.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        include_summary: bool,
        /// Include saved-source annotations throughout this original window.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        include_source_info: bool,
    },
    /// Page only the stored original result prefix, without rescanning or repinning.
    Continue {
        /// Nonzero correlation identity for this operation.
        request_id: u64,
        /// Original operation milliseconds remaining.
        remaining_ms: u64,
        /// The sole session-local window identity.
        window_id: u64,
        /// Next-page size, 1..=64; omitted JSON uses 16.
        #[serde(default = "default_window_page_size")]
        page_size: u8,
    },
    /// Read bounded context around an item already emitted by this same window.
    Expand {
        /// Nonzero correlation identity for this operation.
        request_id: u64,
        /// Original operation milliseconds remaining.
        remaining_ms: u64,
        /// The sole session-local window identity.
        window_id: u64,
        /// Zero-based original retained-vector index, already emitted after flush.
        item_index: u8,
        /// Explicit context bytes before the original range; no default.
        before_bytes: u64,
        /// Explicit context bytes after the original range; no default.
        after_bytes: u64,
    },
    /// Cancel the correlated active operation without allocating a data work slot.
    Cancel {
        /// Active request identity, not a new operation identity.
        request_id: u64,
    },
    /// Release the window and end this child; no subsequent query is admitted.
    Close {
        /// Nonzero close correlation identity.
        request_id: u64,
    },
}

const fn default_window_page_size() -> u8 {
    crate::DEFAULT_WINDOW_PAGE_SIZE
}

const fn default_window_ttl() -> u64 {
    crate::MAX_WINDOW_TTL_MS
}

impl SessionClientFrame {
    /// Correlation identity, with Cancel referring to the active request.
    #[must_use]
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::Open { request_id, .. }
            | Self::Continue { request_id, .. }
            | Self::Expand { request_id, .. }
            | Self::Cancel { request_id }
            | Self::Close { request_id } => *request_id,
        }
    }

    /// Checks closed operation bounds; live window/emitted membership belongs to the owner.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.request_id() == 0 {
            return Err(ProtocolError::InvalidCorrelation);
        }
        let remaining = match self {
            Self::Open {
                remaining_ms,
                query,
                page_size,
                ttl_ms,
                ..
            } => {
                query.validate()?;
                validate_page_size(*page_size)?;
                if *ttl_ms == 0 || *ttl_ms > crate::MAX_WINDOW_TTL_MS {
                    return Err(ProtocolError::InvalidMessage);
                }
                Some(*remaining_ms)
            }
            Self::Continue {
                remaining_ms,
                window_id,
                page_size,
                ..
            } => {
                validate_window_id(*window_id)?;
                validate_page_size(*page_size)?;
                Some(*remaining_ms)
            }
            Self::Expand {
                remaining_ms,
                window_id,
                item_index,
                before_bytes,
                after_bytes,
                ..
            } => {
                validate_window_id(*window_id)?;
                if usize::from(*item_index) >= MAX_ITEMS
                    || before_bytes
                        .checked_add(*after_bytes)
                        .is_none_or(|n| n > crate::MAX_EXPANSION_BYTES)
                {
                    return Err(ProtocolError::InvalidMessage);
                }
                // The owner additionally includes the original span in this ceiling.
                Some(*remaining_ms)
            }
            Self::Cancel { .. } | Self::Close { .. } => None,
        };
        if remaining.is_some_and(|ms| ms == 0 || ms > MAX_REMAINING_MS) {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }
}

fn validate_page_size(size: u8) -> Result<(), ProtocolError> {
    if size == 0 || usize::from(size) > MAX_ITEMS {
        Err(ProtocolError::InvalidMessage)
    } else {
        Ok(())
    }
}

const fn validate_window_id(id: u64) -> Result<(), ProtocolError> {
    if id == crate::SESSION_WINDOW_ID {
        Ok(())
    } else {
        Err(ProtocolError::InvalidMessage)
    }
}

/// An original verified item; its query mode and coordinate domain remain explicit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "mode",
    content = "item",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum WindowItem {
    /// Original ranked card or gap, retaining its backend rank and score.
    Search(SearchItem),
    /// Original raw-byte occurrence or gap.
    Raw(RawExactItem),
    /// Original saved UTF-8 occurrence or gap.
    DecodedUtf8(DecodedUtf8Item),
}

impl WindowItem {
    /// Original saved-source annotation, bound by the enclosing citation/source.
    #[must_use]
    pub const fn source_info(&self) -> Option<&SourceInfo> {
        match self {
            Self::Search(item) => item.source_info(),
            Self::Raw(item) => item.source_info(),
            Self::DecodedUtf8(item) => item.source_info(),
        }
    }
    fn validate(&self, generation: u64) -> Result<(), ProtocolError> {
        match self {
            Self::Search(item) => item.validate(generation),
            Self::Raw(item) => item.validate(),
            Self::DecodedUtf8(item) => item.validate(),
        }
    }
}

/// A stable index into the original bounded vector, usable only after full page flush.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowEntry {
    /// Zero-based original vector index; it is not a new rank or source offset.
    pub item_index: u8,
    /// The original verified card, literal occurrence or explicit gap.
    pub item: WindowItem,
}

/// Original query facts retained independently from page size and total Exact count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSummary {
    /// Original pinned generation, never refreshed by Continue/Expand.
    pub generation: u64,
    /// Stored original result prefix size, at most the original query limit of 64.
    pub retained_items: u8,
    /// Total Exact occurrences, including those outside the stored prefix; None for ranked.
    pub found_matches: Option<u64>,
    /// Original Exact scan completeness; None for ranked retrieval.
    pub scan_complete: Option<bool>,
    /// Original Exact complete-negative claim; None for ranked retrieval.
    pub complete_negative: Option<bool>,
    /// Original query presentation completeness, distinct from a page's output cut.
    pub result_stop: StopReason,
    /// Original Open corpus facts, unchanged throughout this window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corpus: Option<QuerySummary>,
}

/// One whole-item page prepared against the current committed cursor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowPage {
    /// The sole window identity, meaningful only within this child session.
    pub window_id: u64,
    /// Unchanged original query facts.
    pub summary: WindowSummary,
    /// Cursor before this page.
    pub start_index: u8,
    /// Cursor proposed for commitment only after the complete frame flushes.
    pub next_index: u8,
    /// Contiguous whole-item prefix, bounded before decoding a 65th entry.
    #[serde(deserialize_with = "deserialize_items")]
    pub items: Vec<WindowEntry>,
    /// More entries remain in the stored original prefix, not unbounded Exact matches.
    pub has_more: bool,
    /// Complete requested page or a whole-item frame-size cut.
    pub page_stop: StopReason,
}

impl WindowPage {
    /// Requires annotations exactly when the original Open opted in.
    pub fn validate_source_info(&self, requested: bool) -> Result<(), ProtocolError> {
        for entry in &self.items {
            source_info_requested(entry.item.source_info(), requested)?;
        }
        Ok(())
    }
    /// Checks original facts, contiguous indexes and item coordinate bindings.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_window_id(self.window_id)?;
        let summary = &self.summary;
        if let Some(corpus) = &summary.corpus {
            corpus.validate()?;
        }
        if summary.generation == 0
            || usize::from(summary.retained_items) > MAX_ITEMS
            || self.start_index > self.next_index
            || self.next_index > summary.retained_items
            || usize::from(self.next_index - self.start_index) != self.items.len()
            || self.has_more != (self.next_index < summary.retained_items)
            || (self.items.is_empty() && self.has_more)
            || !matches!(
                (
                    summary.found_matches,
                    summary.scan_complete,
                    summary.complete_negative
                ),
                (None, None, None) | (Some(_), Some(_), Some(_))
            )
            || summary.complete_negative.is_some_and(|negative| {
                negative
                    != (summary.scan_complete == Some(true) && summary.found_matches == Some(0))
            })
        {
            return Err(ProtocolError::InvalidMessage);
        }
        for (index, entry) in self.items.iter().enumerate() {
            if usize::from(entry.item_index) != usize::from(self.start_index) + index {
                return Err(ProtocolError::InvalidMessage);
            }
            entry.item.validate(summary.generation)?;
            if matches!(&entry.item, WindowItem::Search(_)) != summary.found_matches.is_none() {
                return Err(ProtocolError::InvalidMessage);
            }
            if let WindowItem::Search(item) = &entry.item
                && usize::from(item.rank()) != usize::from(entry.item_index) + 1
            {
                return Err(ProtocolError::InvalidMessage);
            }
        }
        Ok(())
    }
}

/// Returned context bytes in the original representation domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExpansionContent {
    /// Raw retained bytes, without UTF-8 decoding or replacement.
    Raw {
        /// Exact context bytes in lowercase hex.
        bytes_hex: String,
    },
    /// Exact retained UTF-8 context, preserving BOM and line endings.
    Utf8 {
        /// Exact whole UTF-8 text in the bounded neighborhood.
        text: String,
    },
}

/// Bounded context from an already emitted item's original representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowExpansion {
    /// Same-child window identity.
    pub window_id: u64,
    /// Already emitted original vector index.
    pub item_index: u8,
    /// Pinned original source revision and input `ObjectRef`.
    pub source: RawSourceRef,
    /// Actual saved artifact for UTF-8; None for raw bytes.
    pub prepared: Option<ObjectRefDto>,
    /// Inclusive context start in the original representation.
    pub representation_start: u64,
    /// Exclusive context end in the original representation.
    pub representation_end: u64,
    /// Inclusive mapped source start; current supported mappings are identity.
    pub source_start: u64,
    /// Exclusive mapped source end.
    pub source_end: u64,
    /// Exact verified context, not a latest-source substitution.
    pub content: ExpansionContent,
    /// The original emitted item's cached saved-source annotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_info: Option<SourceInfo>,
}

impl WindowExpansion {
    /// Checks total returned size, representation kind and identity mapping.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if let Some(info) = &self.source_info {
            info.validate()?;
        }
        validate_window_id(self.window_id)?;
        self.source.validate()?;
        let length = self
            .representation_end
            .checked_sub(self.representation_start)
            .filter(|n| *n != 0 && *n <= crate::MAX_EXPANSION_BYTES)
            .ok_or(ProtocolError::InvalidMessage)?;
        if usize::from(self.item_index) >= MAX_ITEMS
            || self.source_start != self.representation_start
            || self.source_end != self.representation_end
            || self.source_end > self.source.input.bytes
        {
            return Err(ProtocolError::InvalidMessage);
        }
        match (&self.content, &self.prepared) {
            (ExpansionContent::Raw { bytes_hex }, None) => {
                if !lower_hex(
                    bytes_hex,
                    usize::try_from(length * 2).map_err(|_| ProtocolError::InvalidMessage)?,
                ) {
                    return Err(ProtocolError::InvalidMessage);
                }
            }
            (ExpansionContent::Utf8 { text }, Some(prepared)) => {
                prepared.validate(ObjectKindDto::Prepared)?;
                if u64::try_from(text.len()).ok() != Some(length) {
                    return Err(ProtocolError::InvalidMessage);
                }
            }
            _ => return Err(ProtocolError::InvalidMessage),
        }
        Ok(())
    }
}

/// One correlated session reply; expiry may be unsolicited with request identity zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionReply {
    /// Active request identity, or zero only for idle expiry.
    pub request_id: u64,
    /// The complete typed outcome of one operation.
    pub outcome: SessionOutcome,
}

/// A whole session operation outcome, independent of existing one-shot terminals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionOutcome {
    /// Open or Continue page; cursor commitment follows full local flush.
    Page {
        /// The whole-item page.
        page: WindowPage,
    },
    /// Verified original-representation neighborhood.
    Expanded {
        /// The bounded context and its retained citation.
        expansion: WindowExpansion,
    },
    /// Explicit Close completed; the child will terminate and be reaped.
    Closed {},
    /// Fixed lifetime expired; access did not renew it.
    Expired {},
    /// The correlated active operation was cancelled.
    Cancelled {},
    /// Healthy pre-output failure leaves existing committed cursor unchanged.
    Failed {
        /// Content-free validated failure code.
        code: FailureCode,
    },
}

impl SessionReply {
    /// Checks correlation and complete payload bounds before exposure.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.request_id == 0 && !matches!(self.outcome, SessionOutcome::Expired { .. }) {
            return Err(ProtocolError::InvalidCorrelation);
        }
        match &self.outcome {
            SessionOutcome::Page { page } => page.validate(),
            SessionOutcome::Expanded { expansion } => expansion.validate(),
            SessionOutcome::Closed {}
            | SessionOutcome::Expired {}
            | SessionOutcome::Cancelled {}
            | SessionOutcome::Failed { .. } => Ok(()),
        }
    }
}

impl WindowItem {
    /// Checks this original item against the session's query mode and literal.
    pub fn validate_request(
        &self,
        query: &WindowQuery,
        generation: u64,
    ) -> Result<(), ProtocolError> {
        self.validate(generation)?;
        let (ids, source) = match (self, query) {
            (Self::Search(_), WindowQuery::Search(_)) => return Ok(()),
            (Self::Raw(item), WindowQuery::Raw(request)) => {
                if let RawExactItem::Match(hit) = item {
                    const DIGITS: &[u8; 16] = b"0123456789abcdef";
                    if hit.end - hit.start != request.literal.len() as u64
                        || hit
                            .bytes_hex
                            .as_bytes()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .zip(request.literal.bytes())
                            .any(|(pair, byte)| {
                                pair[0] != DIGITS[usize::from(byte >> 4)]
                                    || pair[1] != DIGITS[usize::from(byte & 15)]
                            })
                    {
                        return Err(ProtocolError::InvalidMessage);
                    }
                }
                (&request.source_ids, item.source())
            }
            (Self::DecodedUtf8(item), WindowQuery::DecodedUtf8(request)) => {
                if let DecodedUtf8Item::Match(hit) = item
                    && hit.text != request.literal
                {
                    return Err(ProtocolError::InvalidMessage);
                }
                (&request.source_ids, item.source())
            }
            _ => return Err(ProtocolError::InvalidMessage),
        };
        if ids
            .as_ref()
            .is_some_and(|ids| !ids.contains(&source.source_id))
        {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }
}

impl WindowPage {
    /// Checks a successive page against the original query and previously emitted prefix.
    pub fn validate_request(
        &self,
        query: &WindowQuery,
        emitted: &[WindowEntry],
        page_size: u8,
    ) -> Result<(), ProtocolError> {
        self.validate()?;
        query.validate()?;
        if let Some(corpus) = &self.summary.corpus {
            let selection = match query {
                WindowQuery::Search(_) => None,
                WindowQuery::Raw(request) => request.source_ids.as_deref(),
                WindowQuery::DecodedUtf8(request) => request.source_ids.as_deref(),
            };
            corpus.validate_selection(selection)?;
        }
        validate_page_size(page_size)?;
        let expected = usize::from(page_size)
            .min(usize::from(self.summary.retained_items).saturating_sub(emitted.len()));
        if self.start_index as usize != emitted.len()
            || self.summary.retained_items > query.limit()
            || self.items.len() > expected
            || (self.page_stop == StopReason::Complete) != (self.items.len() == expected)
            || matches!(query, WindowQuery::Search(_)) != self.summary.found_matches.is_none()
        {
            return Err(ProtocolError::InvalidMessage);
        }
        let mut found = 0_u64;
        let mut gaps = 0_u64;
        let mut previous: Option<&WindowItem> = None;
        for (index, entry) in emitted.iter().chain(&self.items).enumerate() {
            entry
                .item
                .validate_request(query, self.summary.generation)?;
            match &entry.item {
                WindowItem::Search(item) => {
                    if emitted.iter().chain(&self.items).take(index).any(|prior| {
                        matches!(&prior.item, WindowItem::Search(p) if p.evidence().point_id == item.evidence().point_id)
                    }) {
                        return Err(ProtocolError::InvalidMessage);
                    }
                }
                WindowItem::Raw(RawExactItem::Match(_))
                | WindowItem::DecodedUtf8(DecodedUtf8Item::Match(_)) => found += 1,
                WindowItem::Raw(RawExactItem::Gap(_))
                | WindowItem::DecodedUtf8(DecodedUtf8Item::Gap(_)) => gaps += 1,
            }
            if let Some(left) = previous {
                validate_window_order(left, &entry.item)?;
            }
            previous = Some(&entry.item);
        }
        if let Some(total) = self.summary.found_matches
            && (found > total
                || (gaps > 0 && self.summary.scan_complete == Some(true))
                || (self.next_index == self.summary.retained_items
                    && self.summary.result_stop == StopReason::Complete
                    && (found != total || self.summary.scan_complete != Some(gaps == 0))))
        {
            return Err(ProtocolError::InvalidMessage);
        }
        Ok(())
    }
}

fn validate_window_order(left: &WindowItem, right: &WindowItem) -> Result<(), ProtocolError> {
    let valid = match (left, right) {
        (WindowItem::Search(a), WindowItem::Search(b)) => a.rank().checked_add(1) == Some(b.rank()),
        (WindowItem::Raw(a), WindowItem::Raw(b)) => {
            a.source().source_namespace_id == b.source().source_namespace_id
                && (a.source().source_id < b.source().source_id
                    || (a.source() == b.source()
                        && matches!((a, b), (RawExactItem::Match(a), RawExactItem::Match(b)) if a.start < b.start)))
        }
        (WindowItem::DecodedUtf8(a), WindowItem::DecodedUtf8(b)) => {
            a.source().source_namespace_id == b.source().source_namespace_id
                && (a.source().source_id < b.source().source_id
                    || (a.source() == b.source()
                        && matches!((a, b), (DecodedUtf8Item::Match(a), DecodedUtf8Item::Match(b))
                            if a.prepared == b.prepared && a.representation_start < b.representation_start)))
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ProtocolError::InvalidMessage)
    }
}

impl WindowExpansion {
    /// Binds returned context to the emitted original item and explicit neighborhood request.
    pub fn validate_request(
        &self,
        request: &SessionClientFrame,
        original: &WindowItem,
    ) -> Result<(), ProtocolError> {
        self.validate()?;
        if self.source_info.as_ref() != original.source_info() {
            return Err(ProtocolError::InvalidMessage);
        }
        let SessionClientFrame::Expand {
            window_id,
            item_index,
            before_bytes,
            after_bytes,
            ..
        } = request
        else {
            return Err(ProtocolError::InvalidMessage);
        };
        let (namespace, id, input, prepared, start, end, text, hex, source) = match original {
            WindowItem::Search(SearchItem::Hit(hit)) => {
                let e = &hit.evidence;
                (
                    &e.source_namespace_id,
                    &e.source_id,
                    &e.input,
                    Some(&e.prepared),
                    e.start,
                    e.end,
                    Some(hit.unit_text.as_str()),
                    None,
                    None,
                )
            }
            WindowItem::Raw(RawExactItem::Match(hit)) => (
                &hit.source.source_namespace_id,
                &hit.source.source_id,
                &hit.source.input,
                None,
                hit.start,
                hit.end,
                None,
                Some(hit.bytes_hex.as_str()),
                Some(&hit.source),
            ),
            WindowItem::DecodedUtf8(DecodedUtf8Item::Match(hit)) => (
                &hit.source.source_namespace_id,
                &hit.source.source_id,
                &hit.source.input,
                Some(&hit.prepared),
                hit.representation_start,
                hit.representation_end,
                Some(hit.text.as_str()),
                None,
                Some(&hit.source),
            ),
            _ => return Err(ProtocolError::InvalidMessage),
        };
        let wanted_start = start.saturating_sub(*before_bytes);
        let wanted_end = end
            .checked_add(*after_bytes)
            .ok_or(ProtocolError::InvalidMessage)?
            .min(input.bytes);
        if *window_id != self.window_id
            || *item_index != self.item_index
            || &self.source.source_namespace_id != namespace
            || &self.source.source_id != id
            || &self.source.input != input
            || self.prepared.as_ref() != prepared
            || source.is_some_and(|s| s != &self.source)
            || self.representation_start != wanted_start
            || self.representation_end != wanted_end
        {
            return Err(ProtocolError::InvalidMessage);
        }
        let from =
            usize::try_from(start - wanted_start).map_err(|_| ProtocolError::InvalidMessage)?;
        let to = usize::try_from(end - wanted_start).map_err(|_| ProtocolError::InvalidMessage)?;
        match (&self.content, text, hex) {
            (ExpansionContent::Utf8 { text: expanded }, Some(original), None)
                if expanded.get(from..to) == Some(original) =>
            {
                Ok(())
            }
            (ExpansionContent::Raw { bytes_hex }, None, Some(original))
                if bytes_hex.get(from * 2..to * 2) == Some(original) =>
            {
                Ok(())
            }
            _ => Err(ProtocolError::InvalidMessage),
        }
    }
}

fn deserialize_items<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    struct Items<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> de::Visitor<'de> for Items<T> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("at most 64 query items")
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut items = Vec::new();
            while items.len() < MAX_ITEMS {
                let Some(item) = seq.next_element()? else {
                    return Ok(items);
                };
                items
                    .try_reserve(1)
                    .map_err(|_| de::Error::custom("item allocation failed"))?;
                items.push(item);
            }
            if seq.next_element::<de::IgnoredAny>()?.is_some() {
                return Err(de::Error::custom("too many query items"));
            }
            Ok(items)
        }
    }
    deserializer.deserialize_seq(Items(std::marker::PhantomData))
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
        && value.bytes().any(|b| b != b'0' && b != b'-')
}

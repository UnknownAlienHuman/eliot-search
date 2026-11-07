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
    },
    /// Start the sole raw literal execution on the same serviced connection.
    RawExact {
        /// Exact correlation identity, currently one.
        request_id: u64,
        /// Remaining original operation milliseconds sampled at send.
        remaining_ms: u64,
        /// Literal and pinned-source selection.
        request: RawExactRequest,
    },
    /// Search complete retained UTF-8 text on the same serviced connection.
    DecodedUtf8 {
        /// Exact correlation identity, currently one.
        request_id: u64,
        /// Remaining original execution milliseconds sampled at send.
        remaining_ms: u64,
        /// Literal and selected committed source identities.
        request: DecodedUtf8Request,
    },
    /// Cancel that same query without another work slot.
    Cancel {
        /// Exact active query identity.
        request_id: u64,
    },
}

impl ClientFrame {
    /// Validates correlation, finite duration and backend query bounds.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let request_id = match self {
            Self::Query {
                request_id,
                remaining_ms,
                request,
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
}

impl fmt::Debug for SearchHit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchHit")
            .field("rank", &self.rank)
            .field("score", &self.score)
            .field("evidence", &self.evidence)
            .field("unit_text_bytes", &self.unit_text.len())
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
}

/// An explicitly missing selected raw object; preparation Gaps do not cause this gap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawExactGap {
    /// Pinned source revision and missing object.
    pub source: RawSourceRef,
    /// Only `SOURCE_OBJECT_MISSING`; corrupt bytes are a failed terminal.
    pub code: FailureCode,
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
    /// Exact committed source binding for this item.
    #[must_use]
    pub const fn source(&self) -> &RawSourceRef {
        match self {
            Self::Match(hit) => &hit.source,
            Self::Gap(gap) => &gap.source,
        }
    }

    fn validate(&self) -> Result<(), ProtocolError> {
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
    /// Source binding in the committed inventory.
    #[must_use]
    pub const fn source(&self) -> &RawSourceRef {
        match self {
            Self::Match(hit) => &hit.source,
            Self::Gap(gap) => &gap.source,
        }
    }

    fn validate(&self) -> Result<(), ProtocolError> {
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

/// The sole child-to-parent terminal for request one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    /// Exact query correlation identity.
    pub request_id: u64,
    /// Complete application outcome.
    pub outcome: QueryOutcome,
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
        Ok(())
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

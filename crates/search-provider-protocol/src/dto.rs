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
        Ok(())
    }
}

fn deserialize_items<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<SearchItem>, D::Error> {
    struct Items;
    impl<'de> de::Visitor<'de> for Items {
        type Value = Vec<SearchItem>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("at most 64 ranked query items")
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
    deserializer.deserialize_seq(Items)
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

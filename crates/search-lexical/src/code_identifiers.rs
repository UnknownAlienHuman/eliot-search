//! Immutable XID18/NFC17 identifier analysis, retaining original coordinates.

use core::{fmt, ops::Range};
use std::collections::BTreeMap;

use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue, DigestInputLimit, NonZeroRevision, OpaqueId, blake3_canonical,
};
use unicode_normalization::UnicodeNormalization;

use crate::{LexicalAnalysis, LexicalInput, LexicalReceipt, LexicalToken, TermStatistics};

mod encoding;
mod fixtures;

pub use encoding::{
    AcceptedCodeIdentifiersProfile, CodeEncodingError, CodeIdentifierEncoding,
    accept_code_identifiers_profile, encode_code_document, encode_code_query,
    validate_code_identifiers_pair,
};

/// A closed distinction between retained legacy behavior and the new code profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalProfileMode {
    /// Existing mutable analyzer configuration, retained for real legacy callers.
    Legacy,
    /// The immutable, case-preserving XID18/NFC17 identifier profile.
    CodeIdentifiersV1,
}

/// The additive identifier profile; never an alias for legacy `code_v1`.
pub const CODE_IDENTIFIERS_PROFILE_ID: &str = "lexical:code_identifiers@1";

const XID_DATA: (u8, u8, u8) = (18, 0, 0);
const NFC_DATA: (u8, u8, u8) = (17, 0, 0);
const IDENTITY_LIMIT: usize = 8192;

/// Closed content-free identifier failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentifierError {
    /// Compiled donor data does not match the selected separate data identities.
    DonorVersionMismatch,
    /// The bounded canonical identity could not be computed.
    CanonicalIdentity,
    /// A lower ceiling is zero or exceeds the profile maximum.
    InvalidLimits,
    /// Empty input is unsupported.
    EmptyInput,
    /// Exact input bytes exceed the operation ceiling.
    InputTooLarge,
    /// Original source coordinates are contradictory or overflow.
    CoordinateInvalid,
    /// The input fails the retained binary-control policy.
    BinaryContent,
    /// Original or NFC scalar count exceeds its separate ceiling.
    TokenScalarsExceeded,
    /// Original bytes exceed their ceiling before normalization.
    OriginalBytesExceeded,
    /// NFC bytes exceed their ceiling before append.
    TermBytesExceeded,
    /// Generated original component spans exceed their ceiling.
    ComponentsExceeded,
    /// Raw whole/component attempts exceed the parent or call ceiling.
    ExpansionsExceeded,
    /// Emitted occurrences exceed their ceiling.
    TokensExceeded,
    /// New normalized term keys exceed their ceiling.
    UniqueTermsExceeded,
    /// Profile identity or document/query kind is incompatible.
    ProfileMismatch,
}

impl IdentifierError {
    /// Stable reason, containing no input or native diagnostics.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DonorVersionMismatch => "CODE_IDENTIFIERS_DONOR_VERSION_MISMATCH",
            Self::CanonicalIdentity => "CODE_IDENTIFIERS_CANONICAL_IDENTITY",
            Self::InvalidLimits => "CODE_IDENTIFIERS_INVALID_LIMITS",
            Self::EmptyInput => "CODE_IDENTIFIERS_EMPTY_INPUT",
            Self::InputTooLarge => "CODE_IDENTIFIERS_INPUT_TOO_LARGE",
            Self::CoordinateInvalid => "CODE_IDENTIFIERS_COORDINATE_INVALID",
            Self::BinaryContent => "CODE_IDENTIFIERS_BINARY_CONTENT",
            Self::TokenScalarsExceeded => "CODE_IDENTIFIERS_TOKEN_SCALARS",
            Self::OriginalBytesExceeded => "CODE_IDENTIFIERS_ORIGINAL_BYTES",
            Self::TermBytesExceeded => "CODE_IDENTIFIERS_TERM_BYTES",
            Self::ComponentsExceeded => "CODE_IDENTIFIERS_COMPONENTS",
            Self::ExpansionsExceeded => "CODE_IDENTIFIERS_EXPANSIONS",
            Self::TokensExceeded => "CODE_IDENTIFIERS_TOKENS",
            Self::UniqueTermsExceeded => "CODE_IDENTIFIERS_UNIQUE_TERMS",
            Self::ProfileMismatch => "CODE_IDENTIFIERS_PROFILE_MISMATCH",
        }
    }
}

impl fmt::Display for IdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for IdentifierError {}

/// Constructor-validated lower operation ceilings, never behavior overrides.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodeIdentifierLimits {
    input_bytes: usize,
    tokens: usize,
    unique_terms: usize,
    original_bytes: usize,
    term_bytes: usize,
    raw_scalars: usize,
    nfc_scalars: usize,
    components: usize,
    expansions: usize,
    total_expansions: usize,
}

impl CodeIdentifierLimits {
    /// Exact hard ceilings, bound into the profile identity.
    pub const HARD: Self = Self {
        input_bytes: 8 * 1024 * 1024,
        tokens: 2_000_000,
        unique_terms: 1_000_000,
        original_bytes: 512,
        term_bytes: 1024,
        raw_scalars: 128,
        nfc_scalars: 128,
        components: 128,
        expansions: 129,
        total_expansions: 2_000_000,
    };

    /// Select positive lower ceilings against every independently frozen dimension.
    ///
    /// # Errors
    /// Refuses zero or above-hard values before input traversal.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        input_bytes: usize,
        tokens: usize,
        unique_terms: usize,
        original_bytes: usize,
        term_bytes: usize,
        raw_scalars: usize,
        nfc_scalars: usize,
        components: usize,
        expansions: usize,
        total_expansions: usize,
    ) -> Result<Self, IdentifierError> {
        let value = Self {
            input_bytes,
            tokens,
            unique_terms,
            original_bytes,
            term_bytes,
            raw_scalars,
            nfc_scalars,
            components,
            expansions,
            total_expansions,
        };
        if value
            .values()
            .into_iter()
            .zip(Self::HARD.values())
            .any(|(actual, hard)| actual == 0 || actual > hard)
        {
            return Err(IdentifierError::InvalidLimits);
        }
        Ok(value)
    }

    const fn values(self) -> [usize; 10] {
        [
            self.input_bytes,
            self.tokens,
            self.unique_terms,
            self.original_bytes,
            self.term_bytes,
            self.raw_scalars,
            self.nfc_scalars,
            self.components,
            self.expansions,
            self.total_expansions,
        ]
    }
}

/// Owner-produced immutable profile with exact artifact and fixture-definition identities.
///
/// Fixture definitions and identity matching are not executed qualification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeIdentifiersProfile {
    id: OpaqueId,
    revision: NonZeroRevision,
    fingerprint: Blake3Digest32,
    artifact_set: Blake3Digest32,
    compatibility_fixture: Blake3Digest32,
    collision_fixture: Blake3Digest32,
}

impl CodeIdentifiersProfile {
    /// Construct the sole fixed profile using the shared canonical BLAKE3 owner.
    ///
    /// # Errors
    /// Refuses donor-data disagreement or bounded canonical construction failure.
    pub fn new() -> Result<Self, IdentifierError> {
        if unicode_ident::UNICODE_VERSION != XID_DATA
            || unicode_normalization::UNICODE_VERSION != NFC_DATA
        {
            return Err(IdentifierError::DonorVersionMismatch);
        }
        let artifact_set = identity(
            "eliot/cbor/lexical-code-artifacts/v1",
            &artifact_definition()?,
        )?;
        let compatibility_fixture = identity(
            "eliot/cbor/lexical-code-compatibility/v1",
            &fixtures::compatibility_definition()?,
        )?;
        let collision_fixture = identity(
            "eliot/cbor/lexical-code-collisions/v1",
            &fixtures::collision_definition()?,
        )?;
        let fingerprint = identity(
            "eliot/cbor/lexical-code-identifiers/v1",
            &profile_definition(artifact_set, compatibility_fixture, collision_fixture)?,
        )?;
        Ok(Self {
            id: OpaqueId::new(CODE_IDENTIFIERS_PROFILE_ID)
                .map_err(|_| IdentifierError::CanonicalIdentity)?,
            revision: NonZeroRevision::new(1).map_err(|_| IdentifierError::CanonicalIdentity)?,
            fingerprint,
            artifact_set,
            compatibility_fixture,
            collision_fixture,
        })
    }

    /// Closed selected mode.
    #[must_use]
    pub const fn mode(&self) -> LexicalProfileMode {
        LexicalProfileMode::CodeIdentifiersV1
    }
    /// Stable new-mode identity.
    #[must_use]
    pub const fn id(&self) -> &OpaqueId {
        &self.id
    }
    /// Exact profile revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }
    /// Internally computed full behavior identity.
    #[must_use]
    pub const fn fingerprint(&self) -> Blake3Digest32 {
        self.fingerprint
    }
    /// Canonical descriptor identity of the three exact published donor artifacts.
    #[must_use]
    pub const fn artifact_set_identity(&self) -> Blake3Digest32 {
        self.artifact_set
    }
    /// Authored document/query compatibility definition identity, not a run verdict.
    #[must_use]
    pub const fn compatibility_fixture_identity(&self) -> Blake3Digest32 {
        self.compatibility_fixture
    }
    /// Authored collision corpus definition identity, not a measured acceptance verdict.
    #[must_use]
    pub const fn collision_fixture_identity(&self) -> Blake3Digest32 {
        self.collision_fixture
    }

    fn preflight(
        input: &LexicalInput,
        limits: CodeIdentifierLimits,
    ) -> Result<(), IdentifierError> {
        if input.is_empty() {
            return Err(IdentifierError::EmptyInput);
        }
        if input.len() > limits.input_bytes {
            return Err(IdentifierError::InputTooLarge);
        }
        let length = u64::try_from(input.len()).map_err(|_| IdentifierError::CoordinateInvalid)?;
        if input.source_start.checked_add(length) != Some(input.source_end) {
            return Err(IdentifierError::CoordinateInvalid);
        }
        crate::analyzer::reject_binary_controls(input.bytes())
            .map_err(|_| IdentifierError::BinaryContent)
    }

    /// Analyze immutable original UTF-8; only emitted terms undergo token-local NFC17.
    ///
    /// # Errors
    /// Any bound, binary or coordinate refusal returns no partial successful analysis.
    pub fn analyze(
        &self,
        input: &LexicalInput,
        limits: CodeIdentifierLimits,
    ) -> Result<CodeIdentifierAnalysis, IdentifierError> {
        Self::preflight(input, limits)?;
        let mut output = Output::default();
        let mut start = None;
        let mut valid = false;
        let mut raw_scalars = 0;
        for (offset, character) in input.text().char_indices() {
            if character == '_' || unicode_ident::is_xid_continue(character) {
                if start.is_none() {
                    start = Some(offset);
                    valid = valid_start(character);
                    raw_scalars = 0;
                }
                if valid {
                    raw_scalars += 1;
                    if raw_scalars > limits.raw_scalars {
                        return Err(IdentifierError::TokenScalarsExceeded);
                    }
                    let bytes = offset + character.len_utf8()
                        - start.ok_or(IdentifierError::CoordinateInvalid)?;
                    if bytes > limits.original_bytes {
                        return Err(IdentifierError::OriginalBytesExceeded);
                    }
                }
            } else if let Some(begin) = start.take()
                && valid
            {
                output.parent(input, begin..offset, limits)?;
            }
        }
        if let Some(begin) = start
            && valid
        {
            output.parent(input, begin..input.len(), limits)?;
        }
        let terms = output
            .statistics
            .into_iter()
            .map(|(term, values)| TermStatistics {
                term,
                frequency: values.0,
                first_position: values.1,
                last_position: values.2,
            })
            .collect::<Vec<_>>();
        let receipt = LexicalReceipt {
            source_id: input.source_id.clone(),
            source_revision: input.source_revision,
            unit_ordinal: input.unit_ordinal,
            analyzer_id: self.id.clone(),
            analyzer_revision: self.revision,
            analyzer_fingerprint: self.fingerprint,
            input_bytes: checked_u64(input.len())?,
            candidate_count: output.position,
            emitted_token_count: checked_u64(output.tokens.len())?,
            stop_word_count: 0,
            short_term_count: 0,
            unique_term_count: checked_u64(terms.len())?,
            position_span: output.position,
        };
        Ok(CodeIdentifierAnalysis {
            analysis: LexicalAnalysis {
                tokens: output.tokens,
                terms,
                receipt,
            },
            expansion_attempts: checked_u64(output.attempts)?,
        })
    }
}

/// Complete owner-produced analysis with separately charged raw expansion work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeIdentifierAnalysis {
    analysis: LexicalAnalysis,
    expansion_attempts: u64,
}

impl CodeIdentifierAnalysis {
    /// Readonly original-coordinate tokens, statistics and content-free receipt.
    #[must_use]
    pub const fn analysis(&self) -> &LexicalAnalysis {
        &self.analysis
    }
    /// Actual whole/proper-component attempts before filtering and deduplication.
    #[must_use]
    pub const fn expansion_attempts(&self) -> u64 {
        self.expansion_attempts
    }
}

#[derive(Default)]
struct Output {
    tokens: Vec<LexicalToken>,
    statistics: BTreeMap<String, (u64, u64, u64)>,
    attempts: usize,
    position: u64,
}

impl Output {
    fn parent(
        &mut self,
        input: &LexicalInput,
        range: Range<usize>,
        limits: CodeIdentifierLimits,
    ) -> Result<(), IdentifierError> {
        let original = &input.text()[range.clone()];
        let parts = component_ranges(original, limits)?;
        let proper = parts
            .iter()
            .filter(|part| **part != (0..original.len()))
            .count();
        let attempts = proper + 1;
        if attempts > limits.expansions {
            return Err(IdentifierError::ExpansionsExceeded);
        }
        self.attempts = self
            .attempts
            .checked_add(attempts)
            .ok_or(IdentifierError::ExpansionsExceeded)?;
        if self.attempts > limits.total_expansions {
            return Err(IdentifierError::ExpansionsExceeded);
        }
        let first_token = self.tokens.len();
        self.emit(input, range.clone(), first_token, limits)?;
        for part in parts {
            if part == (0..original.len()) {
                continue;
            }
            if !original[part.clone()]
                .chars()
                .next()
                .is_some_and(valid_start)
            {
                continue;
            }
            self.emit(
                input,
                range.start + part.start..range.start + part.end,
                first_token,
                limits,
            )?;
        }
        self.position = self
            .position
            .checked_add(1)
            .ok_or(IdentifierError::CoordinateInvalid)?;
        Ok(())
    }

    fn emit(
        &mut self,
        input: &LexicalInput,
        range: Range<usize>,
        first_token: usize,
        limits: CodeIdentifierLimits,
    ) -> Result<(), IdentifierError> {
        let original = &input.text()[range.clone()];
        let mut term = String::new();
        let mut scalars = 0;
        for character in original.nfc() {
            scalars += 1;
            if scalars > limits.nfc_scalars {
                return Err(IdentifierError::TokenScalarsExceeded);
            }
            if term
                .len()
                .checked_add(character.len_utf8())
                .is_none_or(|size| size > limits.term_bytes)
            {
                return Err(IdentifierError::TermBytesExceeded);
            }
            term.push(character);
        }
        if self.tokens[first_token..]
            .iter()
            .any(|token| token.term == term)
        {
            return Ok(());
        }
        if self.tokens.len() >= limits.tokens {
            return Err(IdentifierError::TokensExceeded);
        }
        if !self.statistics.contains_key(&term) && self.statistics.len() >= limits.unique_terms {
            return Err(IdentifierError::UniqueTermsExceeded);
        }
        let start = checked_u64(range.start)?;
        let end = checked_u64(range.end)?;
        let source_start = input
            .source_start
            .checked_add(start)
            .ok_or(IdentifierError::CoordinateInvalid)?;
        let source_end = input
            .source_start
            .checked_add(end)
            .ok_or(IdentifierError::CoordinateInvalid)?;
        let original_chars = u32::try_from(original.chars().count())
            .map_err(|_| IdentifierError::CoordinateInvalid)?;
        if let Some(values) = self.statistics.get_mut(&term) {
            values.0 = values
                .0
                .checked_add(1)
                .ok_or(IdentifierError::CoordinateInvalid)?;
            values.2 = self.position;
        } else {
            self.statistics
                .insert(term.clone(), (1, self.position, self.position));
        }
        self.tokens.push(LexicalToken {
            term,
            position: self.position,
            unit_byte_start: start,
            unit_byte_end: end,
            source_byte_start: source_start,
            source_byte_end: source_end,
            original_char_count: original_chars,
        });
        Ok(())
    }
}

fn valid_start(character: char) -> bool {
    character == '_' || unicode_ident::is_xid_start(character)
}

fn component_ranges(
    original: &str,
    limits: CodeIdentifierLimits,
) -> Result<Vec<Range<usize>>, IdentifierError> {
    let mut ranges = Vec::new();
    let mut segment = 0;
    for (offset, character) in original.char_indices() {
        if character == '_' {
            camel_ranges(original, segment..offset, limits, &mut ranges)?;
            segment = offset + 1;
        }
    }
    camel_ranges(original, segment..original.len(), limits, &mut ranges)?;
    Ok(ranges)
}

fn camel_ranges(
    original: &str,
    segment: Range<usize>,
    limits: CodeIdentifierLimits,
    output: &mut Vec<Range<usize>>,
) -> Result<(), IdentifierError> {
    if segment.is_empty() {
        return Ok(());
    }
    let mut start = segment.start;
    let mut previous: Option<char> = None;
    let mut characters = original[segment.clone()].char_indices().peekable();
    while let Some((relative, character)) = characters.next() {
        let next_lower = characters
            .peek()
            .is_some_and(|(_, next)| next.is_ascii_lowercase());
        let split = previous.is_some_and(|prior| {
            character.is_ascii_uppercase()
                && (prior.is_ascii_lowercase()
                    || prior.is_ascii_digit()
                    || (prior.is_ascii_uppercase() && next_lower))
        });
        if split {
            push_component(output, start..segment.start + relative, limits)?;
            start = segment.start + relative;
        }
        previous = Some(character);
    }
    push_component(output, start..segment.end, limits)
}

fn push_component(
    output: &mut Vec<Range<usize>>,
    range: Range<usize>,
    limits: CodeIdentifierLimits,
) -> Result<(), IdentifierError> {
    if output.len() >= limits.components {
        return Err(IdentifierError::ComponentsExceeded);
    }
    output.push(range);
    Ok(())
}

fn checked_u64(value: usize) -> Result<u64, IdentifierError> {
    u64::try_from(value).map_err(|_| IdentifierError::CoordinateInvalid)
}

fn text(value: &str) -> Result<CanonicalValue, IdentifierError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| IdentifierError::CanonicalIdentity)
}
fn array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, IdentifierError> {
    BoundedList::new(values)
        .map(CanonicalValue::Array)
        .map_err(|_| IdentifierError::CanonicalIdentity)
}
fn digest_value(value: Blake3Digest32) -> Result<CanonicalValue, IdentifierError> {
    BoundedBytes::new(value.as_bytes().to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| IdentifierError::CanonicalIdentity)
}
fn identity(domain: &str, value: &CanonicalValue) -> Result<Blake3Digest32, IdentifierError> {
    let domain =
        CanonicalDigestDomain::parse(domain).map_err(|_| IdentifierError::CanonicalIdentity)?;
    let limit =
        DigestInputLimit::new(IDENTITY_LIMIT).map_err(|_| IdentifierError::CanonicalIdentity)?;
    blake3_canonical(&domain, value, limit).map_err(|_| IdentifierError::CanonicalIdentity)
}

fn artifact_definition() -> Result<CanonicalValue, IdentifierError> {
    array(vec![
        text(
            "exact-published-artifact-set@1;SHA256 archive/table descriptors;not BLAKE3 archive bytes",
        )?,
        array(vec![
            text("unicode-ident")?,
            text("1.0.26")?,
            text("18.0.0")?,
            text("d245f478577f809a851594d02313b640fb437e0bb33866753cff937863096954")?,
            text("10598cfee5bcf3a8ac95bde5d9eb747bcb7d6a18ce65b5d393a14376517d4085")?,
            text("no-features;no-normal/build-dependencies")?,
        ])?,
        array(vec![
            text("unicode-normalization")?,
            text("0.1.25")?,
            text("17.0.0")?,
            text("5fd4f6878c9cb28d874b009da9e8d183b5abc80117c40bbd187a1fde336be6e8")?,
            text("177d5f08019cc8e335444fcab61aabb7f6309f158f6ebbd7525c73c0e532ec44")?,
            text("default-features=false;std;NFC")?,
        ])?,
        array(vec![
            text("tinyvec")?,
            text("1.13.3")?,
            text("fd3ca314f692efd6c868f8408f53fe444634a845f96c028b97d35f6a1f79f0ee")?,
            text("alloc;default-empty;no-enabled-normal/build-dependencies")?,
        ])?,
    ])
}

fn profile_definition(
    artifacts: Blake3Digest32,
    compatibility: Blake3Digest32,
    collisions: Blake3Digest32,
) -> Result<CanonicalValue, IdentifierError> {
    let semantics = [
        "maximal-XIDContinue-or-underscore;XIDStart-or-underscore-first;skip-invalid-run-whole",
        "underscore-only-valid;min-scalars1;no-stopwords-stemming-fullfold-NFKC-locale",
        "case-preserve;token-local-NFC17;original-UTF8-coordinates",
        "whole-first;nonempty-underscore-ASCII-camel-acronym-original-fragments;proper-spans-only",
        "ASCII-adjacent-lower-or-digit-before-upper;upper-upper-before-lower",
        "invalid-start-components-filtered-after-attempt-charge;no-suffix-invention",
        "all-non-XIDContinue-delimiters;no-punctuation-compounds",
        "parent-position-shared;first-NFC-duplicate-keeps-original-span;separate-parents-retain-occurrences",
        "raw-attempts-before-NFC-filter-dedup;no-refunds-or-partial-success;VERSIONED_REBUILD",
        "legacy-term-index-v1:FNV-offsetcbf29ce484222325;seed-rotl17;mul100000001b3;seed-rotr(byte&31);mod-index-space",
        "legacy-collision-v1:sum-prior-distinct;floor((terms-indexes)*1000000/terms)",
        "legacy-weighting-v1:document-f64-Raw;query-Binary;positive-finite-f32;sorted-unique-u32",
        "legacy-artifact-fingerprint-v1:four-lane-mixer;profile-identity=shared-CBOR-BLAKE3",
        "Qdrant-sparse-modifier-IDF;no-local-statistics;generation-selects-profile",
    ];
    array(vec![
        CanonicalValue::U64(1),
        text(CODE_IDENTIFIERS_PROFILE_ID)?,
        digest_value(artifacts)?,
        array(
            semantics
                .into_iter()
                .map(text)
                .collect::<Result<Vec<_>, _>>()?,
        )?,
        array(
            CodeIdentifierLimits::HARD
                .values()
                .into_iter()
                .map(|value| checked_u64(value).map(CanonicalValue::U64))
                .collect::<Result<Vec<_>, _>>()?,
        )?,
        crate::sparse::NumericalParameters::CODE_IDENTIFIERS
            .canonical()
            .map_err(|_| IdentifierError::CanonicalIdentity)?,
        digest_value(compatibility)?,
        digest_value(collisions)?,
        text("eliot.cbor.length-first.v1;complete-preimage8192")?,
    ])
}

//! Deterministic bounded lexical analysis for indexed search.
//!
//! The analyzer performs no I/O and owns no index. It converts exact UTF-8 unit
//! text into normalized terms with original byte offsets, position gaps,
//! deterministic term statistics, and a content-free configuration
//! fingerprint. Over-limit or malformed input fails closed instead of silently
//! truncating semantic evidence.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

/// Conservative finite analyzer limits.
pub const DEFAULT_LEXICAL_LIMITS: LexicalLimits = LexicalLimits {
    max_input_bytes: 8 * 1024 * 1024,
    max_tokens: 2_000_000,
    max_unique_terms: 1_000_000,
    max_term_bytes: 1_024,
    max_stop_words: 65_536,
    max_stop_word_bytes: 1_024,
    max_token_chars: 512,
};

/// Closed content-free lexical failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LexicalError {
    /// Limits are zero or internally inconsistent.
    InvalidLimits,
    /// Analyzer identifier is empty.
    EmptyAnalyzerId,
    /// Input unit is empty.
    EmptyInput,
    /// Input exceeds its finite byte ceiling.
    InputTooLarge,
    /// Minimum token character count is zero or above the hard token ceiling.
    InvalidTokenLength,
    /// Stop-word set exceeds its finite item ceiling.
    TooManyStopWords,
    /// Stop word is empty, over limit, or not already normalized.
    InvalidStopWord,
    /// A normalized term exceeds its finite character or byte ceiling.
    TermTooLong,
    /// Emitted token count exceeds its finite ceiling.
    TooManyTokens,
    /// Distinct term count exceeds its finite ceiling.
    TooManyUniqueTerms,
    /// Position or byte-offset conversion overflowed.
    OffsetOverflow,
    /// Input contains a NUL byte or unsupported binary control density.
    BinaryContent,
    /// Token accounting is internally contradictory.
    AccountingMismatch,
}

impl LexicalError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "LEXICAL_INVALID_LIMITS",
            Self::EmptyAnalyzerId => "LEXICAL_EMPTY_ANALYZER_ID",
            Self::EmptyInput => "LEXICAL_EMPTY_INPUT",
            Self::InputTooLarge => "LEXICAL_INPUT_TOO_LARGE",
            Self::InvalidTokenLength => "LEXICAL_INVALID_TOKEN_LENGTH",
            Self::TooManyStopWords => "LEXICAL_TOO_MANY_STOP_WORDS",
            Self::InvalidStopWord => "LEXICAL_INVALID_STOP_WORD",
            Self::TermTooLong => "LEXICAL_TERM_TOO_LONG",
            Self::TooManyTokens => "LEXICAL_TOO_MANY_TOKENS",
            Self::TooManyUniqueTerms => "LEXICAL_TOO_MANY_UNIQUE_TERMS",
            Self::OffsetOverflow => "LEXICAL_OFFSET_OVERFLOW",
            Self::BinaryContent => "LEXICAL_BINARY_CONTENT",
            Self::AccountingMismatch => "LEXICAL_ACCOUNTING_MISMATCH",
        }
    }
}

impl fmt::Display for LexicalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LexicalError {}

/// Finite analyzer limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LexicalLimits {
    /// Maximum exact UTF-8 input bytes.
    pub max_input_bytes: usize,
    /// Maximum emitted non-stop-word tokens.
    pub max_tokens: usize,
    /// Maximum distinct normalized terms.
    pub max_unique_terms: usize,
    /// Maximum normalized term bytes.
    pub max_term_bytes: usize,
    /// Maximum configured stop words.
    pub max_stop_words: usize,
    /// Maximum UTF-8 bytes in one stop word.
    pub max_stop_word_bytes: usize,
    /// Maximum Unicode scalar values in one token.
    pub max_token_chars: usize,
}

impl LexicalLimits {
    /// Validates every finite dimension as non-zero.
    pub const fn validate(self) -> Result<Self, LexicalError> {
        if self.max_input_bytes == 0
            || self.max_tokens == 0
            || self.max_unique_terms == 0
            || self.max_term_bytes == 0
            || self.max_stop_words == 0
            || self.max_stop_word_bytes == 0
            || self.max_token_chars == 0
        {
            Err(LexicalError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Closed token character policy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TokenCharacterPolicy {
    /// Unicode alphabetic and numeric characters; underscore joins terms.
    UnicodeAlphanumericAndUnderscore,
    /// Unicode alphabetic and numeric characters; underscore is a separator.
    UnicodeAlphanumeric,
}

/// Closed case-normalization policy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CaseNormalization {
    /// Preserve original case in the normalized term.
    Preserve,
    /// Apply deterministic Unicode lower-case expansion.
    UnicodeLowercase,
}

/// Immutable bounded lexical analyzer configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalyzerConfig {
    /// Stable analyzer profile identity.
    pub analyzer_id: OpaqueId,
    /// Monotone analyzer profile revision.
    pub revision: NonZeroRevision,
    /// Character inclusion policy.
    pub character_policy: TokenCharacterPolicy,
    /// Case normalization.
    pub case_normalization: CaseNormalization,
    /// Minimum Unicode scalar count for an emitted term.
    pub min_token_chars: usize,
    /// Whether stop words consume positional gaps.
    pub preserve_stop_word_positions: bool,
    stop_words: BTreeSet<String>,
    fingerprint: Blake3Digest32,
}

impl AnalyzerConfig {
    /// Creates and validates a deterministic analyzer profile.
    pub fn new(
        analyzer_id: OpaqueId,
        revision: NonZeroRevision,
        character_policy: TokenCharacterPolicy,
        case_normalization: CaseNormalization,
        min_token_chars: usize,
        preserve_stop_word_positions: bool,
        stop_words: impl IntoIterator<Item = String>,
        fingerprint: Blake3Digest32,
        limits: LexicalLimits,
    ) -> Result<Self, LexicalError> {
        let limits = limits.validate()?;
        if analyzer_id.as_str().is_empty() {
            return Err(LexicalError::EmptyAnalyzerId);
        }
        if min_token_chars == 0 || min_token_chars > limits.max_token_chars {
            return Err(LexicalError::InvalidTokenLength);
        }
        let stop_words = stop_words.into_iter().collect::<BTreeSet<_>>();
        if stop_words.len() > limits.max_stop_words {
            return Err(LexicalError::TooManyStopWords);
        }
        for stop_word in &stop_words {
            if stop_word.is_empty()
                || stop_word.len() > limits.max_stop_word_bytes
                || stop_word.chars().count() > limits.max_token_chars
                || normalize_term(stop_word, case_normalization) != *stop_word
                || !stop_word
                    .chars()
                    .all(|character| is_token_character(character, character_policy))
            {
                return Err(LexicalError::InvalidStopWord);
            }
        }
        Ok(Self {
            analyzer_id,
            revision,
            character_policy,
            case_normalization,
            min_token_chars,
            preserve_stop_word_positions,
            stop_words,
            fingerprint,
        })
    }

    /// Stop words in canonical lexical order.
    pub fn stop_words(&self) -> impl ExactSizeIterator<Item = &str> {
        self.stop_words.iter().map(String::as_str)
    }

    /// Content-free configuration fingerprint supplied by the configuration
    /// owner and bound into indexed projection identity.
    pub const fn fingerprint(&self) -> Blake3Digest32 {
        self.fingerprint
    }

    /// Returns whether a normalized term is configured as a stop word.
    pub fn is_stop_word(&self, normalized: &str) -> bool {
        self.stop_words.contains(normalized)
    }
}

/// Exact source unit presented to the analyzer.
#[derive(Clone, Eq, PartialEq)]
pub struct LexicalInput {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Retained source revision.
    pub source_revision: NonZeroRevision,
    /// Deterministic unit ordinal within that revision.
    pub unit_ordinal: u64,
    /// Inclusive source byte start of the unit.
    pub source_start: u64,
    /// Exclusive source byte end of the unit.
    pub source_end: u64,
    /// Exact unit UTF-8 text.
    text: String,
}

impl LexicalInput {
    /// Creates one exact UTF-8 unit.
    #[must_use]
    pub const fn new(
        source_id: OpaqueId,
        source_revision: NonZeroRevision,
        unit_ordinal: u64,
        source_start: u64,
        source_end: u64,
        text: String,
    ) -> Self {
        Self {
            source_id,
            source_revision,
            unit_ordinal,
            source_start,
            source_end,
            text,
        }
    }

    /// Exact UTF-8 unit text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Exact UTF-8 unit bytes.
    pub fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// Exact UTF-8 byte length.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns whether the unit is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl fmt::Debug for LexicalInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LexicalInput")
            .field("source_id", &self.source_id)
            .field("source_revision", &self.source_revision)
            .field("unit_ordinal", &self.unit_ordinal)
            .field("source_start", &self.source_start)
            .field("source_end", &self.source_end)
            .field("text", &format_args!("<{} UTF-8 bytes>", self.text.len()))
            .finish()
    }
}

/// One normalized emitted token with exact original offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexicalToken {
    /// Normalized term.
    pub term: String,
    /// Zero-based logical token position, including configured stop-word gaps.
    pub position: u64,
    /// Inclusive byte offset within the source unit.
    pub unit_byte_start: u64,
    /// Exclusive byte offset within the source unit.
    pub unit_byte_end: u64,
    /// Inclusive exact source byte offset.
    pub source_byte_start: u64,
    /// Exclusive exact source byte offset.
    pub source_byte_end: u64,
    /// Number of Unicode scalar values in the original token.
    pub original_char_count: u32,
}

/// Deterministic statistics for one normalized term.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TermStatistics {
    /// Normalized term.
    pub term: String,
    /// Number of emitted occurrences in the unit.
    pub frequency: u64,
    /// First emitted logical position.
    pub first_position: u64,
    /// Last emitted logical position.
    pub last_position: u64,
}

/// Content-free lexical analysis receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexicalReceipt {
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Retained source revision.
    pub source_revision: NonZeroRevision,
    /// Deterministic unit ordinal.
    pub unit_ordinal: u64,
    /// Analyzer profile identity.
    pub analyzer_id: OpaqueId,
    /// Analyzer profile revision.
    pub analyzer_revision: NonZeroRevision,
    /// Analyzer configuration fingerprint.
    pub analyzer_fingerprint: Blake3Digest32,
    /// Exact UTF-8 input bytes.
    pub input_bytes: u64,
    /// Number of lexical candidates before stop/min-length filtering.
    pub candidate_count: u64,
    /// Number of emitted tokens.
    pub emitted_token_count: u64,
    /// Number of filtered stop words.
    pub stop_word_count: u64,
    /// Number of filtered short terms.
    pub short_term_count: u64,
    /// Number of distinct emitted normalized terms.
    pub unique_term_count: u64,
    /// Final logical position span, including preserved stop-word gaps.
    pub position_span: u64,
}

/// Complete deterministic lexical analysis result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexicalAnalysis {
    /// Exact emitted tokens in source order.
    pub tokens: Vec<LexicalToken>,
    /// Distinct term statistics in canonical lexical order.
    pub terms: Vec<TermStatistics>,
    /// Content-free analysis receipt.
    pub receipt: LexicalReceipt,
}

/// Analyzes one exact UTF-8 unit deterministically.
pub fn analyze(
    input: LexicalInput,
    config: &AnalyzerConfig,
    limits: LexicalLimits,
) -> Result<LexicalAnalysis, LexicalError> {
    let limits = limits.validate()?;
    validate_input(&input, limits)?;
    reject_binary_controls(input.bytes())?;

    let mut tokens = Vec::new();
    let mut term_stats = BTreeMap::<String, (u64, u64, u64)>::new();
    let mut candidate_count = 0_u64;
    let mut stop_word_count = 0_u64;
    let mut short_term_count = 0_u64;
    let mut position = 0_u64;
    let mut token_start = None;
    let mut token_chars = 0_usize;

    for (byte_index, character) in input.text.char_indices() {
        if is_token_character(character, config.character_policy) {
            if token_start.is_none() {
                token_start = Some(byte_index);
                token_chars = 0;
            }
            token_chars = token_chars
                .checked_add(1)
                .ok_or(LexicalError::OffsetOverflow)?;
            if token_chars > limits.max_token_chars {
                return Err(LexicalError::TermTooLong);
            }
            continue;
        }
        if let Some(start) = token_start.take() {
            emit_candidate(
                &input,
                config,
                limits,
                start,
                byte_index,
                token_chars,
                &mut position,
                &mut candidate_count,
                &mut stop_word_count,
                &mut short_term_count,
                &mut tokens,
                &mut term_stats,
            )?;
        }
    }
    if let Some(start) = token_start {
        emit_candidate(
            &input,
            config,
            limits,
            start,
            input.len(),
            token_chars,
            &mut position,
            &mut candidate_count,
            &mut stop_word_count,
            &mut short_term_count,
            &mut tokens,
            &mut term_stats,
        )?;
    }

    if tokens.len() > limits.max_tokens || term_stats.len() > limits.max_unique_terms {
        return Err(LexicalError::AccountingMismatch);
    }
    let terms = term_stats
        .into_iter()
        .map(|(term, (frequency, first_position, last_position))| TermStatistics {
            term,
            frequency,
            first_position,
            last_position,
        })
        .collect::<Vec<_>>();
    let emitted_token_count =
        u64::try_from(tokens.len()).map_err(|_| LexicalError::OffsetOverflow)?;
    let unique_term_count =
        u64::try_from(terms.len()).map_err(|_| LexicalError::OffsetOverflow)?;
    let input_bytes =
        u64::try_from(input.len()).map_err(|_| LexicalError::OffsetOverflow)?;
    Ok(LexicalAnalysis {
        tokens,
        terms,
        receipt: LexicalReceipt {
            source_id: input.source_id,
            source_revision: input.source_revision,
            unit_ordinal: input.unit_ordinal,
            analyzer_id: config.analyzer_id.clone(),
            analyzer_revision: config.revision,
            analyzer_fingerprint: config.fingerprint(),
            input_bytes,
            candidate_count,
            emitted_token_count,
            stop_word_count,
            short_term_count,
            unique_term_count,
            position_span: position,
        },
    })
}

fn validate_input(input: &LexicalInput, limits: LexicalLimits) -> Result<(), LexicalError> {
    if input.is_empty() {
        return Err(LexicalError::EmptyInput);
    }
    if input.len() > limits.max_input_bytes {
        return Err(LexicalError::InputTooLarge);
    }
    let input_len = u64::try_from(input.len()).map_err(|_| LexicalError::OffsetOverflow)?;
    let expected_end = input
        .source_start
        .checked_add(input_len)
        .ok_or(LexicalError::OffsetOverflow)?;
    if expected_end != input.source_end {
        return Err(LexicalError::AccountingMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_candidate(
    input: &LexicalInput,
    config: &AnalyzerConfig,
    limits: LexicalLimits,
    start: usize,
    end: usize,
    original_char_count: usize,
    position: &mut u64,
    candidate_count: &mut u64,
    stop_word_count: &mut u64,
    short_term_count: &mut u64,
    tokens: &mut Vec<LexicalToken>,
    term_stats: &mut BTreeMap<String, (u64, u64, u64)>,
) -> Result<(), LexicalError> {
    *candidate_count = candidate_count
        .checked_add(1)
        .ok_or(LexicalError::OffsetOverflow)?;
    let original = input
        .text
        .get(start..end)
        .ok_or(LexicalError::AccountingMismatch)?;
    let term = normalize_term(original, config.case_normalization);
    let term_chars = term.chars().count();
    if term.len() > limits.max_term_bytes || term_chars > limits.max_token_chars {
        return Err(LexicalError::TermTooLong);
    }
    let filtered_short = term_chars < config.min_token_chars;
    let filtered_stop = config.is_stop_word(&term);
    if filtered_short {
        *short_term_count = short_term_count
            .checked_add(1)
            .ok_or(LexicalError::OffsetOverflow)?;
    }
    if filtered_stop {
        *stop_word_count = stop_word_count
            .checked_add(1)
            .ok_or(LexicalError::OffsetOverflow)?;
    }
    let consumes_position = !filtered_short
        && (!filtered_stop || config.preserve_stop_word_positions);
    let current_position = *position;
    if consumes_position {
        *position = position
            .checked_add(1)
            .ok_or(LexicalError::OffsetOverflow)?;
    }
    if filtered_short || filtered_stop {
        return Ok(());
    }
    if tokens.len() >= limits.max_tokens {
        return Err(LexicalError::TooManyTokens);
    }
    let start_u64 = u64::try_from(start).map_err(|_| LexicalError::OffsetOverflow)?;
    let end_u64 = u64::try_from(end).map_err(|_| LexicalError::OffsetOverflow)?;
    let source_byte_start = input
        .source_start
        .checked_add(start_u64)
        .ok_or(LexicalError::OffsetOverflow)?;
    let source_byte_end = input
        .source_start
        .checked_add(end_u64)
        .ok_or(LexicalError::OffsetOverflow)?;
    tokens.push(LexicalToken {
        term: term.clone(),
        position: current_position,
        unit_byte_start: start_u64,
        unit_byte_end: end_u64,
        source_byte_start,
        source_byte_end,
        original_char_count: u32::try_from(original_char_count)
            .map_err(|_| LexicalError::OffsetOverflow)?,
    });
    match term_stats.get_mut(&term) {
        Some((frequency, _, last_position)) => {
            *frequency = frequency
                .checked_add(1)
                .ok_or(LexicalError::OffsetOverflow)?;
            *last_position = current_position;
        }
        None => {
            if term_stats.len() >= limits.max_unique_terms {
                return Err(LexicalError::TooManyUniqueTerms);
            }
            term_stats.insert(term, (1, current_position, current_position));
        }
    }
    Ok(())
}

fn is_token_character(character: char, policy: TokenCharacterPolicy) -> bool {
    character.is_alphanumeric()
        || (policy == TokenCharacterPolicy::UnicodeAlphanumericAndUnderscore
            && character == '_')
}

fn normalize_term(value: &str, policy: CaseNormalization) -> String {
    match policy {
        CaseNormalization::Preserve => value.to_owned(),
        CaseNormalization::UnicodeLowercase => value.chars().flat_map(char::to_lowercase).collect(),
    }
}

fn reject_binary_controls(bytes: &[u8]) -> Result<(), LexicalError> {
    if bytes.contains(&0) {
        return Err(LexicalError::BinaryContent);
    }
    let disallowed = bytes
        .iter()
        .filter(|byte| {
            **byte < 0x20 && !matches!(**byte, b'\t' | b'\n' | b'\r' | 0x0c)
        })
        .count();
    let threshold = bytes.len().div_ceil(100).max(4);
    if disallowed >= threshold {
        Err(LexicalError::BinaryContent)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(stop_words: &[&str]) -> AnalyzerConfig {
        AnalyzerConfig::new(
            OpaqueId::new("analyzer:test").expect("analyzer"),
            NonZeroRevision::new(1).expect("revision"),
            TokenCharacterPolicy::UnicodeAlphanumericAndUnderscore,
            CaseNormalization::UnicodeLowercase,
            1,
            true,
            stop_words.iter().map(|word| (*word).to_owned()),
            Blake3Digest32::from_bytes([1; 32]),
            DEFAULT_LEXICAL_LIMITS,
        )
        .expect("config")
    }

    fn input(text: &str) -> LexicalInput {
        LexicalInput::new(
            OpaqueId::new("source:test").expect("source"),
            NonZeroRevision::new(1).expect("revision"),
            0,
            100,
            100 + u64::try_from(text.len()).expect("length"),
            text.to_owned(),
        )
    }

    #[test]
    fn unicode_lowercase_preserves_original_byte_offsets() {
        let result = analyze(
            input("Alpha ΓΆΜΜΑ"),
            &config(&[]),
            DEFAULT_LEXICAL_LIMITS,
        )
        .expect("analyze");
        assert_eq!(result.tokens[0].term, "alpha");
        assert_eq!(result.tokens[0].unit_byte_start, 0);
        assert_eq!(result.tokens[0].unit_byte_end, 5);
        assert_eq!(result.tokens[0].source_byte_start, 100);
        assert_eq!(result.tokens[1].source_byte_start, 106);
    }

    #[test]
    fn stop_words_preserve_position_gaps() {
        let result = analyze(
            input("one and two"),
            &config(&["and"]),
            DEFAULT_LEXICAL_LIMITS,
        )
        .expect("analyze");
        assert_eq!(result.tokens.len(), 2);
        assert_eq!(result.tokens[0].position, 0);
        assert_eq!(result.tokens[1].position, 2);
        assert_eq!(result.receipt.stop_word_count, 1);
        assert_eq!(result.receipt.position_span, 3);
    }

    #[test]
    fn deterministic_term_statistics_are_lexically_ordered() {
        let result = analyze(
            input("beta alpha beta"),
            &config(&[]),
            DEFAULT_LEXICAL_LIMITS,
        )
        .expect("analyze");
        assert_eq!(result.terms[0].term, "alpha");
        assert_eq!(result.terms[1].term, "beta");
        assert_eq!(result.terms[1].frequency, 2);
        assert_eq!(result.terms[1].first_position, 0);
        assert_eq!(result.terms[1].last_position, 2);
    }

    #[test]
    fn underscore_policy_is_explicit() {
        let joined = analyze(
            input("one_two"),
            &config(&[]),
            DEFAULT_LEXICAL_LIMITS,
        )
        .expect("joined");
        assert_eq!(joined.tokens[0].term, "one_two");
        let split_config = AnalyzerConfig::new(
            OpaqueId::new("analyzer:split").expect("analyzer"),
            NonZeroRevision::new(1).expect("revision"),
            TokenCharacterPolicy::UnicodeAlphanumeric,
            CaseNormalization::UnicodeLowercase,
            1,
            true,
            Vec::<String>::new(),
            Blake3Digest32::from_bytes([2; 32]),
            DEFAULT_LEXICAL_LIMITS,
        )
        .expect("config");
        let split = analyze(
            input("one_two"),
            &split_config,
            DEFAULT_LEXICAL_LIMITS,
        )
        .expect("split");
        assert_eq!(
            split
                .tokens
                .iter()
                .map(|token| token.term.as_str())
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
    }

    #[test]
    fn overlong_term_fails_instead_of_truncating() {
        let limits = LexicalLimits {
            max_term_bytes: 3,
            ..DEFAULT_LEXICAL_LIMITS
        };
        assert_eq!(
            analyze(input("four"), &config(&[]), limits),
            Err(LexicalError::TermTooLong)
        );
    }

    #[test]
    fn input_range_must_match_exact_text_length() {
        let mut value = input("abc");
        value.source_end += 1;
        assert_eq!(
            analyze(value, &config(&[]), DEFAULT_LEXICAL_LIMITS),
            Err(LexicalError::AccountingMismatch)
        );
    }

    #[test]
    fn invalid_stop_word_configuration_is_rejected() {
        assert_eq!(
            AnalyzerConfig::new(
                OpaqueId::new("analyzer:test").expect("analyzer"),
                NonZeroRevision::new(1).expect("revision"),
                TokenCharacterPolicy::UnicodeAlphanumeric,
                CaseNormalization::UnicodeLowercase,
                1,
                true,
                ["MixedCase".to_owned()],
                Blake3Digest32::from_bytes([1; 32]),
                DEFAULT_LEXICAL_LIMITS,
            ),
            Err(LexicalError::InvalidStopWord)
        );
    }

    #[test]
    fn binary_controls_fail_closed() {
        assert_eq!(
            analyze(
                input("abc\0def"),
                &config(&[]),
                DEFAULT_LEXICAL_LIMITS,
            ),
            Err(LexicalError::BinaryContent)
        );
    }

    #[test]
    fn input_debug_does_not_dump_source_text() {
        let input = input("sensitive lexical text");
        let debug = format!("{input:?}");
        assert!(!debug.contains("sensitive lexical text"));
        assert!(debug.contains("UTF-8 bytes"));
    }
}

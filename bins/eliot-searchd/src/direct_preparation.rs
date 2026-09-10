//! DIRECT preparation is built during ingestion; search consumes its saved layout.
//! The storage envelope binds the actual namespace/revision/SHA-256 identities.
//!
//! Canonical preparation binding (T16): every durable preparation object is
//! bound to its source revision, representation identity, materializer and
//! unitizer profile revisions and exact digest algorithms with real provenance.
//! No `ReceiptRef` is substituted: representation and profile digests are
//! recomputed from live bytes and validated profiles, never fabricated.

use search_exact::literal::{self, LiteralLimits};
use search_materializer::{
    LineSpan, MaterializationError, MaterializationLimits, materialize_utf8,
};
#[cfg(test)]
use search_unitizer::unitize_text;
use search_unitizer::{SourceLineSpan, UnitSpan, UnitizationError, UnitizationLimits};

use crate::development::{
    MAX_SCAN_INPUT_BYTES, MAX_SCAN_MATCHES, MAX_SCAN_QUERY_BYTES, ScanCoverage, ScanMatch,
    ScanResult,
};
use crate::sha256;

pub const MAX_LAYOUT_BYTES: usize = 64 * 1024 * 1024 - 512;
const MATERIALIZATION: MaterializationLimits = MaterializationLimits {
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    max_output_bytes: MAX_SCAN_INPUT_BYTES,
    max_lines: 1_000_000,
};
const UNITIZATION: UnitizationLimits = UnitizationLimits {
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    preferred_unit_bytes: 16 * 1024,
    max_unit_bytes: 64 * 1024,
    max_lines: 1_000_000,
    max_units: 1_000_000,
};
const LITERAL: LiteralLimits = LiteralLimits {
    max_query_bytes: MAX_SCAN_QUERY_BYTES,
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    max_chunks: 1_000_000,
    max_matches: MAX_SCAN_MATCHES,
};

/// Bind every preparation algorithm/limit, not query options, into the disk key.
pub fn profile_digest() -> [u8; 32] {
    let mut settings = Vec::new();
    for value in [
        MATERIALIZATION.max_input_bytes,
        MATERIALIZATION.max_output_bytes,
        MATERIALIZATION.max_lines,
        UNITIZATION.max_input_bytes,
        UNITIZATION.preferred_unit_bytes,
        UNITIZATION.max_unit_bytes,
        UNITIZATION.max_lines,
        UNITIZATION.max_units,
        MAX_LAYOUT_BYTES,
    ] {
        settings.extend_from_slice(&(value as u64).to_be_bytes());
    }
    sha256::digest_parts(
        b"eliot-search/direct-preparation-profile/v1",
        &[
            b"utf8-exact;no-normalization;crlf-cr-lf;nul-denied;controls-ceil-1pct-min4/v1",
            UnitizationLimits::LAYOUT_FORMAT.as_bytes(),
            &settings,
        ],
    )
}

// ---------------------------------------------------------------------------
// Canonical preparation binding (T16): real provenance, no ReceiptRef.
// ---------------------------------------------------------------------------

/// Canonical DIRECT materializer profile name: exact UTF-8 preservation only.
pub const CANONICAL_MATERIALIZER_NAME: &str = "direct-exact-utf8";
/// Canonical DIRECT materializer profile revision.
pub const CANONICAL_MATERIALIZER_REVISION: u64 = 1;
/// Canonical DIRECT unitizer profile name.
pub const CANONICAL_UNITIZER_NAME: &str = "direct-exact-units";
/// Canonical DIRECT unitizer profile revision.
pub const CANONICAL_UNITIZER_REVISION: u64 = 1;

/// Digest algorithm tags bound into every canonical preparation object.
/// Values match `search-contracts::DigestAlgorithm` wire tags used by the
/// durable unit-manifest codec (`Blake3_256 = 1`, `Sha256 = 2`). Tags are
/// verified on read; unknown tags fail closed instead of reinterpreting bytes.
pub const DIGEST_ALGORITHM_BLAKE3_256: u8 = 1;
/// SHA-256 tag for source content and manifest digests (DIRECT hex identities).
pub const DIGEST_ALGORITHM_SHA256: u8 = 2;

/// Source content uses SHA-256 hex identities (existing DIRECT catalog).
pub const CONTENT_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_SHA256;
/// Representation identities use real BLAKE3 (never SHA-256 relabelled).
pub const REPRESENTATION_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_BLAKE3_256;
/// Manifest envelope digest uses SHA-256 (existing `sha256::digest`).
pub const MANIFEST_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_SHA256;

/// Builds the validated canonical materializer profile for DIRECT.
///
/// Exact UTF-8 preservation only: `Utf8` input, leading-BOM rejection,
/// exact newlines, no normalization and rejection of any lossy transform.
/// DIRECT has no coordinate-map reprojection, so any input requiring a lossy
/// step (BOM strip, transcoding, newline normalization) becomes an explicit
/// preparation gap instead of silently shifted coordinates. Limits match the
/// live `MATERIALIZATION` bounds above, so the digest binds real behavior.
pub fn canonical_materializer_profile()
-> Result<search_materializer::api::ValidatedMaterializerProfile, &'static str> {
    use search_contracts::Blake3Digest32;
    use search_materializer::api::{
        BomPolicy, CoordinateSpace, InvalidSequencePolicy, LossBehavior,
        MaterializationProfileLimits, MaterializerProfileDescriptor, NewlinePolicy, SourceEncoding,
        SourceKind, UnicodeNormalization, validate_materializer_profile,
    };
    let golden = Blake3Digest32::from_bytes(
        *blake3::hash(
            format!("{CANONICAL_MATERIALIZER_NAME}:{CANONICAL_MATERIALIZER_REVISION}").as_bytes(),
        )
        .as_bytes(),
    );
    let descriptor = MaterializerProfileDescriptor {
        profile_name: CANONICAL_MATERIALIZER_NAME.to_owned(),
        profile_revision: CANONICAL_MATERIALIZER_REVISION,
        source_kinds: vec![SourceKind::Text, SourceKind::Code],
        encodings: vec![SourceEncoding::Utf8],
        bom_policy: BomPolicy::RejectWhenPresent,
        invalid_sequence_policy: InvalidSequencePolicy::Reject,
        newline_policy: NewlinePolicy::PreserveExact,
        unicode_normalization: UnicodeNormalization::None,
        loss_behavior: LossBehavior::RejectOnAnyLoss,
        limits: MaterializationProfileLimits {
            max_input_bytes: u64::try_from(MAX_SCAN_INPUT_BYTES)
                .map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID")?,
            max_output_bytes: u64::try_from(MAX_SCAN_INPUT_BYTES)
                .map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID")?,
            max_lines: 1_000_000,
            max_map_segments: 1_000_032,
            max_loss_records: 1_000_032,
            max_steps: 64 * 1024 * 1024,
        },
        coordinate_spaces: vec![
            CoordinateSpace::NativeBytes,
            CoordinateSpace::DecodedScalar,
            CoordinateSpace::CanonicalScalar,
        ],
        golden_fixture_digest: golden,
    };
    validate_materializer_profile(&descriptor).map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID")
}

/// Builds the validated canonical unitizer profile for DIRECT.
///
/// Limits equal the live `UNITIZATION` bounds, so the digest binds the exact
/// boundary decisions used by `encode_layout`/`decode_layout`.
pub fn canonical_unitizer_profile()
-> Result<search_unitizer::ValidatedUnitizerProfile, &'static str> {
    use search_unitizer::{UnitizerProfileDescriptor, validate_unitizer_profile};
    let descriptor = UnitizerProfileDescriptor {
        profile_name: CANONICAL_UNITIZER_NAME.to_owned(),
        profile_revision: CANONICAL_UNITIZER_REVISION,
        limits: UNITIZATION,
    };
    validate_unitizer_profile(&descriptor).map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID")
}

/// Canonical materializer profile digest bytes (real T15 identity, not a receipt).
pub fn canonical_materializer_digest() -> Result<[u8; 32], &'static str> {
    use search_materializer::api::profile_digest;
    Ok(*profile_digest(&canonical_materializer_profile()?).as_bytes())
}

/// Canonical unitizer profile digest bytes (real unitizer identity).
pub fn canonical_unitizer_digest() -> Result<[u8; 32], &'static str> {
    use search_unitizer::unitizer_profile_digest;
    Ok(*unitizer_profile_digest(&canonical_unitizer_profile()?).as_bytes())
}

/// Domain-separated BLAKE3 representation identity over the exact source
/// binding, canonical bytes (or explicit gap reason) and both profile digests.
/// Computed with the real `blake3` crate; never a SHA-256 relabel.
#[allow(clippy::too_many_arguments)]
pub fn representation_id(
    namespace: &[u8; 32],
    source_id: &[u8; 32],
    revision_id: &[u8; 32],
    content_digest: &[u8; 32],
    byte_length: u64,
    materializer_digest: &[u8; 32],
    unitizer_digest: &[u8; 32],
    canonical_or_gap: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-searchd/preparation-representation/v1\x00");
    hasher.update(namespace);
    hasher.update(source_id);
    hasher.update(revision_id);
    hasher.update(content_digest);
    hasher.update(&byte_length.to_be_bytes());
    hasher.update(materializer_digest);
    hasher.update(unitizer_digest);
    hasher.update(&(canonical_or_gap.len() as u64).to_be_bytes());
    hasher.update(canonical_or_gap);
    *hasher.finalize().as_bytes()
}

/// Canonical preparation body plus its representation identity.
///
/// For layout inputs the body equals the existing exact layout encoding, so
/// search coordinates stay source-accurate. A leading BOM is an explicit gap
/// (`DIRECT_REVISION_HAS_BOM`): DIRECT has no coordinate reprojection, so
/// stripping it would silently shift every subsequent offset. All other gaps
/// keep the existing deterministic precedence. No receipt is fabricated.
pub fn encode_canonical_preparation(
    bytes: &[u8],
    namespace: &[u8; 32],
    source_id: &[u8; 32],
    revision_id: &[u8; 32],
    content_digest: &[u8; 32],
    byte_length: u64,
) -> Result<([u8; 32], Vec<u8>), &'static str> {
    let materializer_digest = canonical_materializer_digest()?;
    let unitizer_digest = canonical_unitizer_digest()?;
    // Leading BOM requires a lossy strip under baseline semantics; DIRECT
    // preserves exact offsets, so it reports an explicit gap instead.
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let body = vec![6];
        let representation = representation_id(
            namespace,
            source_id,
            revision_id,
            content_digest,
            byte_length,
            &materializer_digest,
            &unitizer_digest,
            b"DIRECT_REVISION_HAS_BOM",
        );
        return Ok((representation, body));
    }
    let body = encode_preparation(bytes)?;
    let marker: &[u8] = match body.as_slice() {
        [0, layout @ ..] => layout,
        [1] => b"DIRECT_REVISION_NOT_UTF8",
        [2] => b"MATERIALIZATION_BINARY_CONTENT",
        [3] => b"MATERIALIZATION_TOO_MANY_LINES",
        [4] => b"UNITIZATION_TOO_MANY_UNITS",
        [5] => b"DIRECT_PREPARATION_LAYOUT_TOO_LARGE",
        [6] => b"DIRECT_REVISION_HAS_BOM",
        _ => return Err("DIRECT_PREPARATION_INVALID"),
    };
    let representation = representation_id(
        namespace,
        source_id,
        revision_id,
        content_digest,
        byte_length,
        &materializer_digest,
        &unitizer_digest,
        marker,
    );
    Ok((representation, body))
}

/// Canonical preparation receipt: real provenance for one durable object.
/// No `ReceiptRef` is carried: every digest is recomputed from live bytes and
/// validated profiles.
pub struct CanonicalPreparationReceipt {
    /// BLAKE3 representation identity bound to source, body and profiles.
    pub representation_id: [u8; 32],
    /// Canonical materializer profile digest bytes.
    pub materializer_digest: [u8; 32],
    /// Canonical unitizer profile digest bytes.
    pub unitizer_digest: [u8; 32],
    /// Closed preparation gap, if the source is not layout-searchable.
    pub gap: Option<&'static str>,
}

impl CanonicalPreparationReceipt {
    /// Representation identity as lowercase hex.
    pub fn representation_hex(&self) -> String {
        crate::sha256::hex(&self.representation_id)
    }
}

/// Verifies that a stored representation binds the live source bytes and the
/// current canonical profiles. Recomputes the expected identity; mismatch,
/// profile drift or unknown algorithms fail closed without reinterpretation.
pub fn verify_canonical_representation(
    expected: &[u8; 32],
    bytes: &[u8],
    namespace: &[u8; 32],
    source_id: &[u8; 32],
    revision_id: &[u8; 32],
    content_digest: &[u8; 32],
    byte_length: u64,
) -> Result<(), &'static str> {
    let (recomputed, _) = encode_canonical_preparation(
        bytes,
        namespace,
        source_id,
        revision_id,
        content_digest,
        byte_length,
    )?;
    if &recomputed == expected {
        Ok(())
    } else {
        Err("DIRECT_PREPARATION_BINDING_MISMATCH")
    }
}

pub fn validate_query(query: &str) -> Result<(), &'static str> {
    if query.is_empty() {
        return Err("DIRECT_QUERY_EMPTY");
    }
    literal::validate_query(query, LITERAL).map_err(literal::LiteralError::code)
}

// ---------------------------------------------------------------------------
// Canonical durable DIRECT spine gate (T17).
// ---------------------------------------------------------------------------

/// Entire-query corpus budget across all admitted sources.
///
/// Per-file literal limits alone are not a corpus budget: one query must bound
/// total sources, total retained bytes, total emitted matches and total gap
/// records. Ceilings are finite and closed; exhaustion yields explicit typed
/// gaps with `complete = false`, never a narrowed denominator relabelled as
/// success (invariants 6 and 15).
#[allow(clippy::struct_field_names)]
pub struct CorpusBudget {
    /// Maximum admitted sources attempted by one query.
    pub max_sources: usize,
    /// Maximum summed retained bytes attempted by one query.
    pub max_source_bytes: u64,
    /// Maximum emitted matches across every source.
    pub max_matches: usize,
    /// Maximum recorded gap entries.
    pub max_gaps: usize,
}

/// Canonical entire-query budget: 100k sources, 1 GiB retained bytes, 100k
/// matches and 100k gaps. The match/gap ceilings equal the existing shared
/// per-query limits so the budget never widens them.
pub const CANONICAL_CORPUS_BUDGET: CorpusBudget = CorpusBudget {
    max_sources: 100_000,
    max_source_bytes: 1_073_741_824,
    max_matches: MAX_SCAN_MATCHES,
    max_gaps: 100_000,
};

/// Gap reason for admitted sources skipped after the match ceiling filled.
pub const SPINE_GAP_MATCH_LIMIT: &str = "DIRECT_MATCH_LIMIT_REACHED";
/// Gap reason for admitted sources skipped after the corpus byte/source budget.
pub const SPINE_GAP_BUDGET_EXHAUSTED: &str = "DIRECT_CORPUS_BUDGET_EXHAUSTED";
/// Gap reason when a stored match fails source-backed revalidation.
pub const SPINE_GAP_VALIDATION_FAILED: &str = "DIRECT_MATCH_VALIDATION_FAILED";

/// Service-boundary gate over one corpus-search outcome.
///
/// A `complete` claim requires every active source searched, zero gaps and no
/// match-limit truncation. Any partial/degraded outcome must carry
/// `complete = false` and remains typed data, never success (invariant 15).
/// An indexed top-k style narrowing without explicit gaps fails closed
/// (invariant 6).
pub const fn verify_spine_gate(
    active_sources: usize,
    searched_sources: usize,
    gaps_empty: bool,
    complete: bool,
    match_limit_reached: bool,
) -> Result<(), &'static str> {
    if match_limit_reached && complete {
        return Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID");
    }
    if complete && (!gaps_empty || searched_sources != active_sources) {
        return Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID");
    }
    Ok(())
}

/// Revalidates one emitted match against the verified retained revision text.
///
/// The bytes were already verified against the retained content digest before
/// scanning; this step proves the emitted range itself is source-backed: exact
/// bounds, char boundaries and byte equality with the bounded literal query
/// (ASCII folding only). A mismatch fails closed as a per-source gap reason
/// instead of emitting an unproven match. No I/O, no allocation beyond the
/// returned code, no fallback.
pub fn validate_source_backed_match(
    text: &str,
    query: &str,
    ascii_insensitive: bool,
    byte_start: usize,
    byte_end: usize,
) -> Result<(), &'static str> {
    if byte_start >= byte_end || byte_end > text.len() {
        return Err(SPINE_GAP_VALIDATION_FAILED);
    }
    if !text.is_char_boundary(byte_start) || !text.is_char_boundary(byte_end) {
        return Err(SPINE_GAP_VALIDATION_FAILED);
    }
    let Some(slice) = text.get(byte_start..byte_end) else {
        return Err(SPINE_GAP_VALIDATION_FAILED);
    };
    let matched = if ascii_insensitive {
        slice.len() == query.len() && slice.as_bytes().eq_ignore_ascii_case(query.as_bytes())
    } else {
        slice == query
    };
    if matched {
        Ok(())
    } else {
        Err(SPINE_GAP_VALIDATION_FAILED)
    }
}

/// Canonical layout or a closed deterministic preparation gap, never source text.
/// Storage failures are not encoded as content outcomes.
pub fn encode_preparation(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    // Keep the existing DIRECT precedence for invalid UTF-8 versus binary policy.
    if std::str::from_utf8(bytes).is_err() {
        return Ok(vec![1]);
    }
    let prepared = match materialize_utf8(bytes.to_vec(), MATERIALIZATION) {
        Ok(value) => value,
        Err(MaterializationError::BinaryContent) => return Ok(vec![2]),
        Err(MaterializationError::TooManyLines) => return Ok(vec![3]),
        Err(error) => return Err(error.code()),
    };
    let encoded = match UNITIZATION.encode_layout(
        prepared.text(),
        &line_spans(prepared.lines()),
        MAX_LAYOUT_BYTES,
    ) {
        Ok(value) => value,
        Err(UnitizationError::TooManyUnits) => return Ok(vec![4]),
        Err(UnitizationError::InputTooLarge) => return Ok(vec![5]),
        Err(error) => return Err(error.code()),
    };
    let mut output = Vec::with_capacity(encoded.len() + 1);
    output.push(0);
    output.extend_from_slice(&encoded);
    Ok(output)
}

/// Search verified bytes using the saved exact layout. No preparation/storage write.
pub fn scan_prepared(
    text: &str,
    encoded: &[u8],
    query: &str,
    ascii_insensitive: bool,
) -> Result<ScanResult, &'static str> {
    validate_query(query)?;
    if let Some(reason) = preparation_gap(encoded)? {
        return Err(reason);
    }
    let layout = encoded.get(1..).ok_or("DIRECT_PREPARATION_INVALID")?;
    let (lines, units) = UNITIZATION
        .decode_layout(text, layout, MAX_LAYOUT_BYTES)
        .map_err(UnitizationError::code)?;
    scan_layout(text, &lines, &units, query, ascii_insensitive, LITERAL)
}

/// Decode only the outcome framing; source/layout validation remains mandatory for reads.
/// Reused by storage acknowledgements so a saved unsupported input is not reported as a layout.
pub const fn preparation_gap(encoded: &[u8]) -> Result<Option<&'static str>, &'static str> {
    match encoded {
        [0, layout @ ..] if !layout.is_empty() => Ok(None),
        [1] => Ok(Some("DIRECT_REVISION_NOT_UTF8")),
        [2] => Ok(Some("MATERIALIZATION_BINARY_CONTENT")),
        [3] => Ok(Some("MATERIALIZATION_TOO_MANY_LINES")),
        [4] => Ok(Some("UNITIZATION_TOO_MANY_UNITS")),
        [5] => Ok(Some("DIRECT_PREPARATION_LAYOUT_TOO_LARGE")),
        [6] => Ok(Some("DIRECT_REVISION_HAS_BOM")),
        _ => Err("DIRECT_PREPARATION_INVALID"),
    }
}

fn line_spans(lines: &[LineSpan]) -> Vec<SourceLineSpan> {
    lines
        .iter()
        .map(|line| SourceLineSpan {
            line_index: line.line_index,
            source_start: line.source_start,
            source_end: line.source_end,
            content_end: line.content_end,
        })
        .collect()
}

fn scan_layout(
    text: &str,
    lines: &[SourceLineSpan],
    units: &[UnitSpan],
    query: &str,
    ascii_insensitive: bool,
    limits: LiteralLimits,
) -> Result<ScanResult, &'static str> {
    let chunks = units
        .iter()
        .map(|unit| {
            text.get(unit.source_start..unit.source_end)
                .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = literal::scan_chunks(&chunks, query, ascii_insensitive, limits)
        .map_err(literal::LiteralError::code)?;
    let complete = result.complete();
    let matches = result
        .matches
        .into_iter()
        .map(|range| {
            let start =
                u64::try_from(range.start).map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?;
            let line = lines
                .get(lines.partition_point(|line| line.source_end <= start))
                .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")?;
            let line_start = usize::try_from(line.source_start)
                .map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?;
            Ok(ScanMatch {
                byte_start: range.start,
                byte_end: range.end,
                line: usize::try_from(line.line_index)
                    .map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?,
                column_bytes: range
                    .start
                    .checked_sub(line_start)
                    .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")?,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    Ok(ScanResult {
        matches,
        coverage: ScanCoverage {
            input_bytes: result.input_bytes,
            complete,
            match_limit_reached: result.match_limit_reached,
        },
    })
}

// Preserve the existing pure regression entrypoint; product queries use scan_prepared.
#[cfg(test)]
pub fn prepare_and_scan(
    text: String,
    query: &str,
    insensitive: bool,
) -> Result<ScanResult, &'static str> {
    validate_query(query)?;
    scan_with_limits(
        text,
        query,
        insensitive,
        MATERIALIZATION,
        UNITIZATION,
        LITERAL,
    )
}
#[cfg(test)]
fn scan_with_limits(
    text: String,
    query: &str,
    insensitive: bool,
    materialization: MaterializationLimits,
    unitization: UnitizationLimits,
    literal: LiteralLimits,
) -> Result<ScanResult, &'static str> {
    let prepared =
        materialize_utf8(text.into_bytes(), materialization).map_err(MaterializationError::code)?;
    let lines = line_spans(prepared.lines());
    let units =
        unitize_text(prepared.text(), &lines, unitization).map_err(UnitizationError::code)?;
    scan_layout(prepared.text(), &lines, &units, query, insensitive, literal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small(text: &str, query: &str) -> ScanResult {
        scan_with_limits(
            text.to_owned(),
            query,
            false,
            MATERIALIZATION,
            UnitizationLimits {
                preferred_unit_bytes: 2,
                max_unit_bytes: 4,
                ..UNITIZATION
            },
            LITERAL,
        )
        .unwrap()
    }

    #[test]
    fn query_longer_than_units_still_matches_once() {
        let result = small("a123456789z", "123456789");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(
            (result.matches[0].byte_start, result.matches[0].byte_end),
            (1, 10)
        );
        assert!(result.coverage.complete);
    }

    #[test]
    fn newline_and_unicode_coordinates_are_source_byte_coordinates() {
        let text = "a\r\nβ\rc\n𐀀 target";
        let result = small(text, "target");
        let start = text.find("target").unwrap();
        assert_eq!(
            result.matches,
            vec![ScanMatch {
                byte_start: start,
                byte_end: start + 6,
                line: 3,
                column_bytes: 5
            }]
        );
        let crossing = small("a\r\nβ", "\r\nβ");
        assert_eq!(
            (crossing.matches[0].byte_start, crossing.matches[0].line),
            (1, 0)
        );
    }

    #[test]
    fn repeated_matches_across_units_are_not_duplicated_or_lost() {
        let result = small("aaaaa", "aaa");
        assert_eq!(
            result
                .matches
                .iter()
                .map(|item| item.byte_start)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn preparation_failure_is_not_complete_empty_success() {
        assert_eq!(
            prepare_and_scan("a\0b".to_owned(), "missing", false),
            Err("MATERIALIZATION_BINARY_CONTENT")
        );
        assert!(small("", "missing").coverage.complete);
    }

    #[test]
    fn limits_propagate_without_claiming_full_coverage() {
        let result = scan_with_limits(
            "aaaa".to_owned(),
            "aa",
            false,
            MATERIALIZATION,
            UNITIZATION,
            LiteralLimits {
                max_matches: 1,
                ..LITERAL
            },
        )
        .unwrap();
        assert_eq!(result.matches.len(), 1);
        assert!(!result.coverage.complete);
        assert!(result.coverage.match_limit_reached);
        assert_eq!(
            scan_with_limits(
                "a\nb".to_owned(),
                "b",
                false,
                MaterializationLimits {
                    max_lines: 1,
                    ..MATERIALIZATION
                },
                UNITIZATION,
                LITERAL
            ),
            Err("MATERIALIZATION_TOO_MANY_LINES")
        );
    }

    fn canonical_ids() -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
        ([1; 32], [2; 32], [3; 32], [4; 32])
    }

    #[test]
    fn canonical_profiles_validate_with_deterministic_digests() {
        let materializer = canonical_materializer_profile().expect("materializer");
        let unitizer = canonical_unitizer_profile().expect("unitizer");
        assert_eq!(materializer.revision(), CANONICAL_MATERIALIZER_REVISION);
        assert_eq!(unitizer.revision(), CANONICAL_UNITIZER_REVISION);
        assert_eq!(
            canonical_materializer_digest().expect("digest"),
            *search_materializer::api::profile_digest(&materializer).as_bytes()
        );
        assert_eq!(
            canonical_unitizer_digest().expect("digest"),
            *search_unitizer::unitizer_profile_digest(&unitizer).as_bytes()
        );
        // Same inputs yield byte-identical identities (restart-safe).
        assert_eq!(
            canonical_materializer_digest().expect("first"),
            canonical_materializer_digest().expect("second")
        );
        assert_eq!(
            canonical_unitizer_digest().expect("first"),
            canonical_unitizer_digest().expect("second")
        );
    }

    #[test]
    fn digest_algorithm_tags_are_explicit_and_distinct() {
        assert_eq!(CONTENT_DIGEST_ALGORITHM, DIGEST_ALGORITHM_SHA256);
        assert_eq!(REPRESENTATION_DIGEST_ALGORITHM, DIGEST_ALGORITHM_BLAKE3_256);
        assert_eq!(MANIFEST_DIGEST_ALGORITHM, DIGEST_ALGORITHM_SHA256);
        assert_ne!(
            REPRESENTATION_DIGEST_ALGORITHM, CONTENT_DIGEST_ALGORITHM,
            "BLAKE3 representation must never relabel SHA-256 content"
        );
    }

    #[test]
    fn leading_bom_is_an_explicit_gap_not_a_silent_layout() {
        let (namespace, source, revision, content) = canonical_ids();
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(b"needle");
        let (representation, body) =
            encode_canonical_preparation(&with_bom, &namespace, &source, &revision, &content, 9)
                .expect("bom");
        assert_eq!(body, vec![6]);
        assert_eq!(preparation_gap(&body), Ok(Some("DIRECT_REVISION_HAS_BOM")));
        verify_canonical_representation(
            &representation,
            &with_bom,
            &namespace,
            &source,
            &revision,
            &content,
            9,
        )
        .expect("verify bom");
        // Same bytes without the BOM take the layout path with a new identity.
        let (plain_repr, plain_body) =
            encode_canonical_preparation(b"needle", &namespace, &source, &revision, &content, 6)
                .expect("plain");
        assert!(preparation_gap(&plain_body).expect("gap").is_none());
        assert_ne!(representation, plain_repr);
    }

    #[test]
    fn representation_binds_bytes_profiles_and_rejects_tamper() {
        let (namespace, source, revision, content) = canonical_ids();
        let (first, _) =
            encode_canonical_preparation(b"same", &namespace, &source, &revision, &content, 4)
                .expect("first");
        let (second, _) =
            encode_canonical_preparation(b"same", &namespace, &source, &revision, &content, 4)
                .expect("second");
        assert_eq!(first, second, "deterministic across restarts");
        let (changed, _) =
            encode_canonical_preparation(b"same!", &namespace, &source, &revision, &content, 5)
                .expect("changed");
        assert_ne!(first, changed);
        // Tampered bytes, swapped revision or truncated length fail closed.
        assert_eq!(
            verify_canonical_representation(
                &first,
                b"tampered",
                &namespace,
                &source,
                &revision,
                &content,
                8
            ),
            Err("DIRECT_PREPARATION_BINDING_MISMATCH")
        );
        assert_eq!(
            verify_canonical_representation(
                &first, b"same", &namespace, &source, &[9; 32], &content, 4
            ),
            Err("DIRECT_PREPARATION_BINDING_MISMATCH")
        );
    }

    #[test]
    fn spine_gate_accepts_complete_denominator_and_rejects_narrowing() {
        assert!(verify_spine_gate(3, 3, true, true, false).is_ok());
        assert!(verify_spine_gate(0, 0, true, true, false).is_ok());
        // Partial/degraded stays typed incomplete: allowed, never success.
        assert!(verify_spine_gate(3, 2, false, false, false).is_ok());
        assert!(verify_spine_gate(3, 2, true, false, true).is_ok());
        // Complete with gaps, unsearched sources or truncation fails closed.
        assert_eq!(
            verify_spine_gate(3, 3, false, true, false),
            Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID")
        );
        assert_eq!(
            verify_spine_gate(3, 2, true, true, false),
            Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID")
        );
        assert_eq!(
            verify_spine_gate(3, 3, true, true, true),
            Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID")
        );
    }

    #[test]
    fn corpus_budget_never_widens_shared_per_query_limits() {
        const {
            assert!(CANONICAL_CORPUS_BUDGET.max_sources > 0);
            assert!(CANONICAL_CORPUS_BUDGET.max_source_bytes > 0);
        }
        assert_eq!(CANONICAL_CORPUS_BUDGET.max_matches, MAX_SCAN_MATCHES);
        assert_eq!(CANONICAL_CORPUS_BUDGET.max_gaps, 100_000);
    }

    #[test]
    fn source_backed_match_validation_rejects_unproven_ranges() {
        assert!(validate_source_backed_match("needle here", "needle", false, 0, 6).is_ok());
        assert!(validate_source_backed_match("xNEEDLE", "needle", true, 1, 7).is_ok());
        // Wrong bytes, out-of-bounds, empty and non-char-boundary ranges fail.
        assert!(validate_source_backed_match("needle here", "needle", false, 1, 7).is_err());
        assert!(validate_source_backed_match("needle", "needle", false, 0, 7).is_err());
        assert!(validate_source_backed_match("needle", "needle", false, 0, 0).is_err());
        assert!(validate_source_backed_match("needle", "needle", false, 2, 2).is_err());
        assert!(validate_source_backed_match("βγ", "β", false, 0, 1).is_err());
        assert!(validate_source_backed_match("needle", "NEEDLE", false, 0, 6).is_err());
    }

    #[test]
    fn truncated_and_reordered_bodies_fail_closed() {
        assert_eq!(preparation_gap(&[]), Err("DIRECT_PREPARATION_INVALID"));
        assert_eq!(preparation_gap(&[0]), Err("DIRECT_PREPARATION_INVALID"));
        assert_eq!(preparation_gap(&[7]), Err("DIRECT_PREPARATION_INVALID"));
        // Gap bytes are fixed singletons; trailing bytes are never reinterpreted.
        assert_eq!(preparation_gap(&[1, 0]), Err("DIRECT_PREPARATION_INVALID"));
        assert_eq!(preparation_gap(&[6, 6]), Err("DIRECT_PREPARATION_INVALID"));
    }
}

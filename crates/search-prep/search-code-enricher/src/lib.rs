//! No-execute tolerant Rust structural enrichment.
//!
//! The built-in baseline scans immutable UTF-8 representation bytes and emits
//! bounded syntax facts, roles, configuration predicates, and descriptive
//! relations. It never invokes Cargo, rustc, build scripts, procedural macros,
//! language servers, shell commands, network resources, or repository code.
//! Its assurance is explicitly tolerant syntax, never compiler truth.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    AssuranceClass, Blake3Digest32, BoundedList, EntityKind, EvidenceRole,
    MAX_LIST_ITEMS, OpaqueId, ProfileId, ReceiptRef, RepresentationId,
    SourceRevisionRef, UnitId,
};

/// Maximum UTF-8 bytes retained for one public identifier or attribute value.
pub const MAX_STRUCTURAL_TEXT_BYTES: usize = 1_024;

/// Closed enrichment failure surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EnrichError {
    /// Parser profile is malformed or internally contradictory.
    ParserProfileInvalid,
    /// Exact qualification evidence does not bind the profile.
    ParserNotQualified,
    /// Selected parser implementation is unavailable.
    ParserUnavailable,
    /// Input encoding or Rust dialect is unsupported.
    RustInputUnsupported,
    /// Input exceeds the accepted byte ceiling.
    RustInputTooLarge,
    /// Tolerant recovery occurred and a complete parse was required.
    ParseDegraded,
    /// Explicit cancellation was observed.
    ParseCancelled,
    /// Finite byte, line, node, relation, depth, or step budget was exhausted.
    ParseBudgetExhausted,
    /// A fact could not be mapped to exact representation coordinates.
    StructuralFactUnmapped,
    /// A relation target cannot be resolved without compiler semantics.
    StructuralRelationAmbiguous,
    /// Configuration expression is malformed or unsupported.
    ConfigurationAmbiguous,
    /// Coordinate-map/profile/revision identity mismatch.
    AnchorMappingFailed,
    /// Manifest counts, identities, or digests are contradictory.
    EnrichmentManifestInvalid,
    /// Existing facts were produced by another profile.
    EnrichmentProfileMismatch,
    /// A bounded shared-contract construction failed.
    ContractViolation,
}

impl EnrichError {
    /// Stable machine-readable code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ParserProfileInvalid => "PARSER_PROFILE_INVALID",
            Self::ParserNotQualified => "PARSER_NOT_QUALIFIED",
            Self::ParserUnavailable => "PARSER_UNAVAILABLE",
            Self::RustInputUnsupported => "RUST_INPUT_UNSUPPORTED",
            Self::RustInputTooLarge => "RUST_INPUT_TOO_LARGE",
            Self::ParseDegraded => "PARSE_DEGRADED",
            Self::ParseCancelled => "PARSE_CANCELLED",
            Self::ParseBudgetExhausted => "PARSE_BUDGET_EXHAUSTED",
            Self::StructuralFactUnmapped => "STRUCTURAL_FACT_UNMAPPED",
            Self::StructuralRelationAmbiguous => "STRUCTURAL_RELATION_AMBIGUOUS",
            Self::ConfigurationAmbiguous => "CONFIGURATION_AMBIGUOUS",
            Self::AnchorMappingFailed => "ANCHOR_MAPPING_FAILED",
            Self::EnrichmentManifestInvalid => "ENRICHMENT_MANIFEST_INVALID",
            Self::EnrichmentProfileMismatch => "ENRICHMENT_PROFILE_MISMATCH",
            Self::ContractViolation => "ENRICHMENT_CONTRACT_VIOLATION",
        }
    }
}

impl fmt::Display for EnrichError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for EnrichError {}

/// Rust edition covered by a parser profile.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustEdition {
    /// Rust 2015.
    Rust2015,
    /// Rust 2018.
    Rust2018,
    /// Rust 2021.
    Rust2021,
    /// Rust 2024.
    Rust2024,
}

/// Tolerant malformed-input policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MalformedInputPolicy {
    /// Return bounded recovery nodes and explicit degradation gaps.
    RecoverWithGaps,
    /// Reject the representation on the first malformed construct.
    Reject,
}

/// Exact immutable parser/enrichment profile candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustParserProfile {
    /// Stable profile identifier.
    pub profile_id: ProfileId,
    /// Exact parser package name.
    pub parser_package: ProfileId,
    /// Exact parser version or immutable revision.
    pub parser_version: ProfileId,
    /// Source artifact checksum.
    pub source_checksum: Blake3Digest32,
    /// License evidence.
    pub license_receipt_ref: ReceiptRef,
    /// Node schema digest.
    pub node_schema_digest: Blake3Digest32,
    /// Query/extraction schema digest.
    pub query_schema_digest: Blake3Digest32,
    /// Golden fixture corpus digest.
    pub golden_fixture_digest: Blake3Digest32,
    /// Supported Rust editions.
    pub supported_editions: BTreeSet<RustEdition>,
    /// Maximum input bytes.
    pub max_input_bytes: usize,
    /// Maximum syntax nodes.
    pub max_nodes: usize,
    /// Maximum structural depth.
    pub max_depth: usize,
    /// Maximum attributes retained for one node.
    pub max_attributes_per_node: usize,
    /// Maximum diagnostics.
    pub max_diagnostics: usize,
    /// Malformed-input behavior.
    pub malformed_policy: MalformedInputPolicy,
    /// Mandatory declaration that parsing cannot execute repository code.
    pub no_execute: bool,
    /// Digest of all behavior-affecting profile fields.
    pub profile_digest: Blake3Digest32,
}

/// Qualification evidence for one exact profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParserQualificationReceipt {
    /// Accepted profile digest.
    pub profile_digest: Blake3Digest32,
    /// Accepted source checksum.
    pub source_checksum: Blake3Digest32,
    /// Accepted golden fixture digest.
    pub golden_fixture_digest: Blake3Digest32,
    /// Independent no-execute audit receipt.
    pub no_execute_audit_receipt_ref: ReceiptRef,
    /// Qualification receipt identity.
    pub qualification_receipt_ref: ReceiptRef,
}

/// Validated parser profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedRustParserProfile {
    profile: RustParserProfile,
    qualification: ParserQualificationReceipt,
}

impl QualifiedRustParserProfile {
    /// Validated profile.
    #[must_use]
    pub const fn profile(&self) -> &RustParserProfile {
        &self.profile
    }

    /// Exact qualification receipt.
    #[must_use]
    pub const fn qualification(&self) -> &ParserQualificationReceipt {
        &self.qualification
    }
}

/// Validates exact parser identity, limits, dialects, evidence, and no-execute policy.
pub fn validate_parser_profile(
    profile: RustParserProfile,
    qualification: ParserQualificationReceipt,
) -> Result<QualifiedRustParserProfile, EnrichError> {
    let floating = ["latest", "*", "^", "~", ">", "<"]
        .iter()
        .any(|marker| profile.parser_version.as_str().contains(marker));
    let limits_valid = profile.max_input_bytes > 0
        && profile.max_nodes > 0
        && profile.max_nodes <= MAX_LIST_ITEMS
        && profile.max_depth > 0
        && profile.max_depth <= 256
        && profile.max_attributes_per_node > 0
        && profile.max_attributes_per_node <= MAX_LIST_ITEMS
        && profile.max_diagnostics > 0
        && profile.max_diagnostics <= MAX_LIST_ITEMS;
    if floating
        || profile.supported_editions.is_empty()
        || !limits_valid
        || !profile.no_execute
    {
        return Err(EnrichError::ParserProfileInvalid);
    }
    if qualification.profile_digest != profile.profile_digest
        || qualification.source_checksum != profile.source_checksum
        || qualification.golden_fixture_digest != profile.golden_fixture_digest
    {
        return Err(EnrichError::ParserNotQualified);
    }
    Ok(QualifiedRustParserProfile {
        profile,
        qualification,
    })
}

/// Immutable code representation consumed by the enricher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustRepresentation {
    /// Exact source revision.
    pub source_revision_ref: SourceRevisionRef,
    /// Representation identity.
    pub representation_id: RepresentationId,
    /// Coordinate-map digest.
    pub coordinate_map_digest: Blake3Digest32,
    /// Exact representation-content digest.
    pub representation_digest: Blake3Digest32,
    /// Rust edition selected by the admitted profile.
    pub edition: RustEdition,
    /// Exact immutable UTF-8 representation bytes.
    pub bytes: Vec<u8>,
    /// Units covering this representation in source order.
    pub unit_ids: BoundedList<UnitId, MAX_LIST_ITEMS>,
}

/// Finite parser/extraction budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnrichmentBudget {
    /// Maximum bytes examined.
    pub max_input_bytes: usize,
    /// Maximum source lines examined.
    pub max_lines: usize,
    /// Maximum syntax nodes.
    pub max_nodes: usize,
    /// Maximum facts.
    pub max_facts: usize,
    /// Maximum relations.
    pub max_relations: usize,
    /// Maximum configuration depth.
    pub max_configuration_depth: usize,
    /// Maximum scanner steps.
    pub max_steps: u64,
}

impl EnrichmentBudget {
    /// Validates non-zero finite limits.
    pub fn validate(self) -> Result<Self, EnrichError> {
        let valid = self.max_input_bytes > 0
            && self.max_lines > 0
            && self.max_nodes > 0
            && self.max_nodes <= MAX_LIST_ITEMS
            && self.max_facts > 0
            && self.max_facts <= MAX_LIST_ITEMS
            && self.max_relations > 0
            && self.max_relations <= MAX_LIST_ITEMS
            && self.max_configuration_depth > 0
            && self.max_configuration_depth <= 256
            && self.max_steps > 0;
        if valid {
            Ok(self)
        } else {
            Err(EnrichError::ParseBudgetExhausted)
        }
    }
}

/// Explicit cancellation observation.
pub trait EnrichmentCancellation {
    /// Whether cancellation is currently requested.
    fn is_cancelled(&self) -> bool;
}

/// Cancellation implementation that never cancels.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancelled;

impl EnrichmentCancellation for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Baseline syntax-node class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RustSyntaxKind {
    /// Module declaration.
    Module,
    /// Free function.
    Function,
    /// Method-like function inside an impl/trait block.
    Method,
    /// Struct, enum, union, or type alias.
    Type,
    /// Trait declaration.
    Trait,
    /// Impl block.
    Impl,
    /// Constant.
    Constant,
    /// Static.
    Static,
    /// Macro definition.
    MacroDefinition,
    /// Macro invocation.
    MacroInvocation,
    /// Test function or module.
    Test,
    /// Documentation section/comment associated with the following node.
    Documentation,
    /// Configuration attribute.
    Configuration,
    /// Unclassified tolerant recovery node.
    Unknown,
}

/// Exact byte/line range in one representation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TextRange {
    /// Inclusive byte start.
    pub byte_start: u64,
    /// Exclusive byte end.
    pub byte_end: u64,
    /// Inclusive zero-based line start.
    pub line_start: u64,
    /// Exclusive zero-based line end.
    pub line_end: u64,
}

impl TextRange {
    /// Validates a non-empty range within the representation.
    pub fn validate(self, representation_len: usize) -> Result<(), EnrichError> {
        let end = usize::try_from(self.byte_end)
            .map_err(|_| EnrichError::StructuralFactUnmapped)?;
        if self.byte_start >= self.byte_end
            || self.line_start >= self.line_end
            || end > representation_len
        {
            Err(EnrichError::StructuralFactUnmapped)
        } else {
            Ok(())
        }
    }
}

/// Bounded parser diagnostic class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ParseDiagnosticKind {
    /// Braces are not balanced in the bounded scan.
    UnbalancedDelimiter,
    /// Declaration keyword has no recognizable identifier.
    MissingIdentifier,
    /// Configuration attribute could not be parsed.
    MalformedConfiguration,
    /// A line was retained only as a tolerant recovery node.
    RecoveryNode,
    /// Scanner stopped at a finite budget.
    BudgetBoundary,
}

/// Content-free parse diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseDiagnostic {
    /// Diagnostic class.
    pub kind: ParseDiagnosticKind,
    /// Exact source range.
    pub range: TextRange,
}

/// Tolerant baseline syntax node. It contains no vendor parser type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSyntaxNode {
    /// Deterministic node digest.
    pub node_digest: Blake3Digest32,
    /// Closed syntax kind.
    pub kind: RustSyntaxKind,
    /// Bounded identifier when present.
    pub name: Option<String>,
    /// Exact representation range.
    pub range: TextRange,
    /// Bounded raw attribute spellings.
    pub attributes: BoundedList<String, MAX_LIST_ITEMS>,
    /// Whether a documentation comment immediately precedes the node.
    pub documented: bool,
    /// Whether the tolerant scanner recovered this node from malformed input.
    pub recovered: bool,
    /// Approximate lexical nesting depth, not compiler scope identity.
    pub lexical_depth: usize,
}

/// Parse completeness state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseState {
    /// Bounded scan completed without recovery diagnostics.
    CompleteTolerantSyntax,
    /// Scan completed with explicit recovery diagnostics.
    DegradedTolerantSyntax,
    /// Cancellation or a hard budget prevented completion.
    Incomplete,
}

/// Vendor-neutral tolerant syntax tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustSyntaxTree {
    /// Source revision.
    pub source_revision_ref: SourceRevisionRef,
    /// Representation identity.
    pub representation_id: RepresentationId,
    /// Exact coordinate-map digest.
    pub coordinate_map_digest: Blake3Digest32,
    /// Exact qualified profile digest.
    pub parser_profile_digest: Blake3Digest32,
    /// Syntax nodes in source order.
    pub nodes: BoundedList<RustSyntaxNode, MAX_LIST_ITEMS>,
    /// Explicit bounded diagnostics.
    pub diagnostics: BoundedList<ParseDiagnostic, MAX_LIST_ITEMS>,
    /// Parse completeness.
    pub state: ParseState,
    /// Scanner steps consumed.
    pub steps: u64,
}

/// Parses immutable Rust text with the no-execute tolerant baseline.
pub fn parse_rust_no_execute(
    representation: &RustRepresentation,
    profile: &QualifiedRustParserProfile,
    budget: EnrichmentBudget,
    cancellation: &dyn EnrichmentCancellation,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<RustSyntaxTree, EnrichError> {
    let budget = budget.validate()?;
    if !profile.profile.supported_editions.contains(&representation.edition) {
        return Err(EnrichError::RustInputUnsupported);
    }
    if representation.bytes.len() > profile.profile.max_input_bytes
        || representation.bytes.len() > budget.max_input_bytes
    {
        return Err(EnrichError::RustInputTooLarge);
    }
    let text = core::str::from_utf8(&representation.bytes)
        .map_err(|_| EnrichError::RustInputUnsupported)?;
    let line_count = text.lines().count().max(1);
    if line_count > budget.max_lines {
        return Err(EnrichError::ParseBudgetExhausted);
    }

    let mut nodes = Vec::new();
    let mut diagnostics = Vec::new();
    let mut pending_attributes = Vec::new();
    let mut pending_documentation = false;
    let mut offset = 0_usize;
    let mut depth = 0_usize;
    let mut steps = 0_u64;

    for (line_index, raw_line) in text.split_inclusive('\n').enumerate() {
        if cancellation.is_cancelled() {
            return Err(EnrichError::ParseCancelled);
        }
        steps = steps
            .checked_add(u64::try_from(raw_line.len()).unwrap_or(u64::MAX))
            .ok_or(EnrichError::ParseBudgetExhausted)?;
        if steps > budget.max_steps {
            return Err(EnrichError::ParseBudgetExhausted);
        }
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start();
        let leading = line.len().saturating_sub(trimmed.len());
        let line_start = offset;
        let content_start = line_start.saturating_add(leading);
        let content_end = line_start.saturating_add(line.len());
        let line_number = u64::try_from(line_index)
            .map_err(|_| EnrichError::ParseBudgetExhausted)?;
        let range = TextRange {
            byte_start: u64::try_from(content_start)
                .map_err(|_| EnrichError::StructuralFactUnmapped)?,
            byte_end: u64::try_from(content_end.max(content_start.saturating_add(1)))
                .map_err(|_| EnrichError::StructuralFactUnmapped)?,
            line_start: line_number,
            line_end: line_number
                .checked_add(1)
                .ok_or(EnrichError::StructuralFactUnmapped)?,
        };

        if trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || trimmed.starts_with("/**")
            || trimmed.starts_with("/*!")
        {
            pending_documentation = true;
            offset = offset.saturating_add(raw_line.len());
            continue;
        }
        if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            if pending_attributes.len() >= profile.profile.max_attributes_per_node {
                return Err(EnrichError::ParseBudgetExhausted);
            }
            if trimmed.len() > MAX_STRUCTURAL_TEXT_BYTES {
                return Err(EnrichError::RustInputUnsupported);
            }
            pending_attributes.push(trimmed.to_owned());
            offset = offset.saturating_add(raw_line.len());
            continue;
        }

        let classification = classify_line(trimmed, depth, &pending_attributes);
        if let Some(mut classified) = classification {
            if nodes.len() >= profile.profile.max_nodes || nodes.len() >= budget.max_nodes {
                return Err(EnrichError::ParseBudgetExhausted);
            }
            if classified.name.as_ref().is_some_and(|name| {
                name.is_empty() || name.len() > MAX_STRUCTURAL_TEXT_BYTES
            }) {
                classified.name = None;
                classified.recovered = true;
                push_diagnostic(
                    &mut diagnostics,
                    profile,
                    ParseDiagnosticKind::MissingIdentifier,
                    range,
                )?;
            }
            let digest_input = node_digest_input(
                representation,
                profile,
                classified.kind,
                classified.name.as_deref(),
                range,
                &pending_attributes,
                pending_documentation,
                classified.recovered,
                depth,
            )?;
            nodes.push(RustSyntaxNode {
                node_digest: Blake3Digest32::from_bytes(blake3_256(&digest_input)),
                kind: classified.kind,
                name: classified.name,
                range,
                attributes: BoundedList::new(core::mem::take(&mut pending_attributes))
                    .map_err(|_| EnrichError::ContractViolation)?,
                documented: pending_documentation,
                recovered: classified.recovered,
                lexical_depth: depth,
            });
            pending_documentation = false;
        } else if !trimmed.is_empty()
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("/*")
            && looks_structural(trimmed)
        {
            if profile.profile.malformed_policy == MalformedInputPolicy::Reject {
                return Err(EnrichError::ParseDegraded);
            }
            push_diagnostic(
                &mut diagnostics,
                profile,
                ParseDiagnosticKind::RecoveryNode,
                range,
            )?;
        }

        let closes = trimmed.bytes().filter(|byte| *byte == b'}').count();
        depth = depth.saturating_sub(closes);
        let opens = trimmed.bytes().filter(|byte| *byte == b'{').count();
        depth = depth.saturating_add(opens);
        if depth > profile.profile.max_depth || depth > budget.max_configuration_depth {
            return Err(EnrichError::ParseBudgetExhausted);
        }
        offset = offset.saturating_add(raw_line.len());
    }

    if depth != 0 {
        let final_offset = u64::try_from(representation.bytes.len())
            .map_err(|_| EnrichError::StructuralFactUnmapped)?;
        let range = TextRange {
            byte_start: final_offset.saturating_sub(1),
            byte_end: final_offset.max(1),
            line_start: u64::try_from(line_count.saturating_sub(1)).unwrap_or(u64::MAX),
            line_end: u64::try_from(line_count).unwrap_or(u64::MAX),
        };
        if profile.profile.malformed_policy == MalformedInputPolicy::Reject {
            return Err(EnrichError::ParseDegraded);
        }
        push_diagnostic(
            &mut diagnostics,
            profile,
            ParseDiagnosticKind::UnbalancedDelimiter,
            range,
        )?;
    }
    if !pending_attributes.is_empty() {
        let final_offset = u64::try_from(representation.bytes.len())
            .map_err(|_| EnrichError::StructuralFactUnmapped)?;
        push_diagnostic(
            &mut diagnostics,
            profile,
            ParseDiagnosticKind::RecoveryNode,
            TextRange {
                byte_start: final_offset.saturating_sub(1),
                byte_end: final_offset.max(1),
                line_start: u64::try_from(line_count.saturating_sub(1)).unwrap_or(u64::MAX),
                line_end: u64::try_from(line_count).unwrap_or(u64::MAX),
            },
        )?;
    }
    if nodes.len() > budget.max_nodes || diagnostics.len() > profile.profile.max_diagnostics {
        return Err(EnrichError::ParseBudgetExhausted);
    }
    let state = if diagnostics.is_empty() {
        ParseState::CompleteTolerantSyntax
    } else {
        ParseState::DegradedTolerantSyntax
    };
    Ok(RustSyntaxTree {
        source_revision_ref: representation.source_revision_ref,
        representation_id: representation.representation_id,
        coordinate_map_digest: representation.coordinate_map_digest,
        parser_profile_digest: profile.profile.profile_digest,
        nodes: BoundedList::new(nodes).map_err(|_| EnrichError::ContractViolation)?,
        diagnostics: BoundedList::new(diagnostics)
            .map_err(|_| EnrichError::ContractViolation)?,
        state,
        steps,
    })
}

struct ClassifiedLine {
    kind: RustSyntaxKind,
    name: Option<String>,
    recovered: bool,
}

fn classify_line(
    line: &str,
    depth: usize,
    attributes: &[String],
) -> Option<ClassifiedLine> {
    let line = strip_visibility_and_qualifiers(line);
    let is_test = attributes.iter().any(|attribute| {
        attribute.starts_with("#[test")
            || attribute.contains("cfg(test)")
            || attribute.contains("cfg_attr(test")
    });
    let declarations = [
        ("fn ", if depth > 0 { RustSyntaxKind::Method } else { RustSyntaxKind::Function }),
        ("struct ", RustSyntaxKind::Type),
        ("enum ", RustSyntaxKind::Type),
        ("union ", RustSyntaxKind::Type),
        ("type ", RustSyntaxKind::Type),
        ("trait ", RustSyntaxKind::Trait),
        ("mod ", RustSyntaxKind::Module),
        ("const ", RustSyntaxKind::Constant),
        ("static ", RustSyntaxKind::Static),
        ("impl ", RustSyntaxKind::Impl),
        ("macro_rules! ", RustSyntaxKind::MacroDefinition),
        ("macro ", RustSyntaxKind::MacroDefinition),
    ];
    for (prefix, mut kind) in declarations {
        if let Some(rest) = line.strip_prefix(prefix) {
            if is_test && matches!(kind, RustSyntaxKind::Function | RustSyntaxKind::Method | RustSyntaxKind::Module) {
                kind = RustSyntaxKind::Test;
            }
            let name = parse_identifier(rest);
            return Some(ClassifiedLine {
                kind,
                recovered: name.is_none(),
                name,
            });
        }
    }
    if let Some(name) = parse_macro_invocation(line) {
        return Some(ClassifiedLine {
            kind: RustSyntaxKind::MacroInvocation,
            name: Some(name),
            recovered: false,
        });
    }
    None
}

fn strip_visibility_and_qualifiers(mut line: &str) -> &str {
    loop {
        let trimmed = line.trim_start();
        let prefixes = [
            "pub(crate) ",
            "pub(super) ",
            "pub(self) ",
            "pub ",
            "async ",
            "const ",
            "unsafe ",
            "extern \"C\" ",
            "extern \"Rust\" ",
            "default ",
        ];
        if let Some(prefix) = prefixes.iter().find(|prefix| trimmed.starts_with(**prefix)) {
            line = &trimmed[prefix.len()..];
        } else {
            return trimmed;
        }
    }
}

fn parse_identifier(value: &str) -> Option<String> {
    let value = value.trim_start();
    let identifier = value
        .chars()
        .take_while(|character| {
            character.is_ascii_alphanumeric() || *character == '_' || *character == '#'
        })
        .collect::<String>();
    if identifier.is_empty() {
        None
    } else {
        Some(identifier.trim_start_matches("r#").to_owned())
    }
}

fn parse_macro_invocation(line: &str) -> Option<String> {
    let bang = line.find('!')?;
    let name = line[..bang].trim();
    if name.is_empty()
        || name.contains(char::is_whitespace)
        || !line[bang + 1..].trim_start().starts_with(['(', '[', '{'])
    {
        return None;
    }
    parse_identifier(name)
}

fn looks_structural(line: &str) -> bool {
    [
        "fn ", "struct ", "enum ", "trait ", "impl ", "mod ", "type ",
        "const ", "static ", "macro ", "macro_rules!",
    ]
    .iter()
    .any(|needle| line.contains(needle))
}

fn push_diagnostic(
    diagnostics: &mut Vec<ParseDiagnostic>,
    profile: &QualifiedRustParserProfile,
    kind: ParseDiagnosticKind,
    range: TextRange,
) -> Result<(), EnrichError> {
    if diagnostics.len() >= profile.profile.max_diagnostics {
        return Err(EnrichError::ParseBudgetExhausted);
    }
    diagnostics.push(ParseDiagnostic { kind, range });
    Ok(())
}

fn node_digest_input(
    representation: &RustRepresentation,
    profile: &QualifiedRustParserProfile,
    kind: RustSyntaxKind,
    name: Option<&str>,
    range: TextRange,
    attributes: &[String],
    documented: bool,
    recovered: bool,
    depth: usize,
) -> Result<Vec<u8>, EnrichError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/rust-syntax-node/v1")?;
    bytes.extend_from_slice(representation.source_revision_ref.revision_id.as_bytes());
    bytes.extend_from_slice(representation.representation_id.as_bytes());
    bytes.extend_from_slice(profile.profile.profile_digest.as_bytes());
    bytes.push(syntax_kind_tag(kind));
    append(&mut bytes, name.unwrap_or("").as_bytes())?;
    bytes.extend_from_slice(&range.byte_start.to_be_bytes());
    bytes.extend_from_slice(&range.byte_end.to_be_bytes());
    bytes.extend_from_slice(&range.line_start.to_be_bytes());
    bytes.extend_from_slice(&range.line_end.to_be_bytes());
    bytes.extend_from_slice(&u64::try_from(depth).unwrap_or(u64::MAX).to_be_bytes());
    bytes.push(u8::from(documented));
    bytes.push(u8::from(recovered));
    for attribute in attributes {
        append(&mut bytes, attribute.as_bytes())?;
    }
    Ok(bytes)
}

/// Canonical configuration predicate AST.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ConfigurationPredicate {
    /// `all(...)` conjunction.
    All(BoundedList<ConfigurationPredicate, MAX_LIST_ITEMS>),
    /// `any(...)` disjunction.
    Any(BoundedList<ConfigurationPredicate, MAX_LIST_ITEMS>),
    /// `not(...)` negation.
    Not(Box<ConfigurationPredicate>),
    /// Bare key such as `unix` or `test`.
    Key(String),
    /// Key-value predicate such as `target_os = "windows"`.
    KeyValue {
        /// Predicate key.
        key: String,
        /// Exact value without quotes.
        value: String,
    },
    /// Malformed or unsupported configuration expression.
    Unknown,
}

/// Extracted predicate plus exact evidence range and digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationObservation {
    /// Canonical predicate.
    pub predicate: ConfigurationPredicate,
    /// Attribute range.
    pub range: TextRange,
    /// Predicate digest.
    pub predicate_digest: Blake3Digest32,
    /// Whether parsing was complete.
    pub complete: bool,
}

/// Parses `cfg` and the controlling predicate of `cfg_attr` without evaluation.
pub fn extract_configuration_predicate(
    attribute: &str,
    range: TextRange,
    max_depth: usize,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<Option<ConfigurationObservation>, EnrichError> {
    let trimmed = attribute.trim();
    let body = if let Some(body) = trimmed
        .strip_prefix("#[cfg(")
        .and_then(|value| value.strip_suffix(")]"))
    {
        Some(body)
    } else if let Some(body) = trimmed
        .strip_prefix("#[cfg_attr(")
        .and_then(|value| value.strip_suffix(")]"))
    {
        split_top_level(body)?
            .into_iter()
            .next()
            .map(str::trim)
    } else {
        None
    };
    let Some(body) = body else {
        return Ok(None);
    };
    let predicate = parse_cfg_expression(body, 0, max_depth)?;
    let complete = predicate != ConfigurationPredicate::Unknown;
    let canonical = canonical_cfg(&predicate)?;
    Ok(Some(ConfigurationObservation {
        predicate,
        range,
        predicate_digest: Blake3Digest32::from_bytes(blake3_256(&canonical)),
        complete,
    }))
}

fn parse_cfg_expression(
    value: &str,
    depth: usize,
    max_depth: usize,
) -> Result<ConfigurationPredicate, EnrichError> {
    if depth > max_depth {
        return Err(EnrichError::ParseBudgetExhausted);
    }
    let value = value.trim();
    for (name, constructor) in [
        (
            "all",
            ConfigurationPredicate::All
                as fn(BoundedList<ConfigurationPredicate, MAX_LIST_ITEMS>)
                    -> ConfigurationPredicate,
        ),
        (
            "any",
            ConfigurationPredicate::Any
                as fn(BoundedList<ConfigurationPredicate, MAX_LIST_ITEMS>)
                    -> ConfigurationPredicate,
        ),
    ] {
        if let Some(inner) = value
            .strip_prefix(name)
            .and_then(str::trim_start)
            .strip_prefix('(')
            .and_then(|body| body.strip_suffix(')'))
        {
            let items = split_top_level(inner)?
                .into_iter()
                .map(|item| parse_cfg_expression(item, depth.saturating_add(1), max_depth))
                .collect::<Result<Vec<_>, _>>()?;
            if items.is_empty() {
                return Ok(ConfigurationPredicate::Unknown);
            }
            return BoundedList::new(items)
                .map(constructor)
                .map_err(|_| EnrichError::ParseBudgetExhausted);
        }
    }
    if let Some(inner) = value
        .strip_prefix("not")
        .and_then(str::trim_start)
        .strip_prefix('(')
        .and_then(|body| body.strip_suffix(')'))
    {
        return Ok(ConfigurationPredicate::Not(Box::new(parse_cfg_expression(
            inner,
            depth.saturating_add(1),
            max_depth,
        )?)));
    }
    if let Some((key, raw_value)) = value.split_once('=') {
        let key = key.trim();
        let raw_value = raw_value.trim();
        let Some(unquoted) = raw_value
            .strip_prefix('"')
            .and_then(|body| body.strip_suffix('"'))
        else {
            return Ok(ConfigurationPredicate::Unknown);
        };
        if valid_cfg_atom(key) && valid_cfg_value(unquoted) {
            return Ok(ConfigurationPredicate::KeyValue {
                key: key.to_owned(),
                value: unquoted.to_owned(),
            });
        }
        return Ok(ConfigurationPredicate::Unknown);
    }
    if valid_cfg_atom(value) {
        Ok(ConfigurationPredicate::Key(value.to_owned()))
    } else {
        Ok(ConfigurationPredicate::Unknown)
    }
}

fn split_top_level(value: &str) -> Result<Vec<&str>, EnrichError> {
    let mut output = Vec::new();
    let mut depth = 0_i64;
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0_usize;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quoted {
            escaped = true;
            continue;
        }
        if character == '"' {
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err(EnrichError::ConfigurationAmbiguous);
                }
            }
            ',' if depth == 0 => {
                output.push(&value[start..index]);
                start = index.saturating_add(1);
            }
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return Err(EnrichError::ConfigurationAmbiguous);
    }
    output.push(&value[start..]);
    Ok(output)
}

fn valid_cfg_atom(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_STRUCTURAL_TEXT_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_cfg_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_STRUCTURAL_TEXT_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"')
}

fn canonical_cfg(value: &ConfigurationPredicate) -> Result<Vec<u8>, EnrichError> {
    let mut output = Vec::new();
    encode_cfg(value, &mut output)?;
    Ok(output)
}

fn encode_cfg(
    value: &ConfigurationPredicate,
    output: &mut Vec<u8>,
) -> Result<(), EnrichError> {
    match value {
        ConfigurationPredicate::All(items) => {
            output.push(1);
            for item in items {
                encode_cfg(item, output)?;
            }
        }
        ConfigurationPredicate::Any(items) => {
            output.push(2);
            for item in items {
                encode_cfg(item, output)?;
            }
        }
        ConfigurationPredicate::Not(item) => {
            output.push(3);
            encode_cfg(item, output)?;
        }
        ConfigurationPredicate::Key(key) => {
            output.push(4);
            append(output, key.as_bytes())?;
        }
        ConfigurationPredicate::KeyValue { key, value } => {
            output.push(5);
            append(output, key.as_bytes())?;
            append(output, value.as_bytes())?;
        }
        ConfigurationPredicate::Unknown => output.push(6),
    }
    Ok(())
}

/// Provider assurance that deliberately avoids compiler-truth claims.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderAssurance {
    /// Complete bounded tolerant syntax scan.
    TolerantSyntax,
    /// Tolerant scan with explicit recovery diagnostics.
    DegradedTolerantSyntax,
    /// Parsing/extraction did not complete.
    Unavailable,
}

/// Maps parse state to a conservative provider assurance.
#[must_use]
pub const fn assurance_for(state: ParseState) -> ProviderAssurance {
    match state {
        ParseState::CompleteTolerantSyntax => ProviderAssurance::TolerantSyntax,
        ParseState::DegradedTolerantSyntax => ProviderAssurance::DegradedTolerantSyntax,
        ParseState::Incomplete => ProviderAssurance::Unavailable,
    }
}

/// Closed structural gap.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EnrichmentGap {
    /// Parser recovery affected one or more nodes.
    MalformedSource,
    /// Configuration expression could not be parsed.
    ConfigurationUnknown,
    /// Relation target remains name-only and ambiguous.
    AmbiguousRelationTarget,
    /// Budget omitted facts or relations.
    BudgetOmission,
    /// Coordinate mapping is not exact.
    AnchorMappingGap,
}

/// Deterministic structural fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralFact {
    /// Stable fact digest.
    pub fact_digest: Blake3Digest32,
    /// Exact source revision.
    pub source_revision_ref: SourceRevisionRef,
    /// Representation identity.
    pub representation_id: RepresentationId,
    /// Optional containing unit.
    pub unit_id: Option<UnitId>,
    /// Closed entity kind.
    pub entity_kind: EntityKind,
    /// Bounded name.
    pub name: Option<String>,
    /// Exact representation range.
    pub range: TextRange,
    /// Evidence role.
    pub evidence_role: EvidenceRole,
    /// Parsed configuration predicate.
    pub configuration: Option<ConfigurationObservation>,
    /// Qualified parser profile digest.
    pub parser_profile_digest: Blake3Digest32,
    /// Contract assurance ceiling. Tolerant syntax is descriptive only.
    pub assurance: AssuranceClass,
    /// Explicit gaps attached to this fact.
    pub gaps: BoundedList<EnrichmentGap, MAX_LIST_ITEMS>,
}

/// Classifies one syntax node as evidence without inferring compiler semantics.
#[must_use]
pub fn classify_evidence_role(node: &RustSyntaxNode) -> EvidenceRole {
    match node.kind {
        RustSyntaxKind::Test => EvidenceRole::Test,
        RustSyntaxKind::Documentation => EvidenceRole::Documentation,
        RustSyntaxKind::Configuration => EvidenceRole::Configuration,
        RustSyntaxKind::MacroInvocation => EvidenceRole::Reference,
        RustSyntaxKind::Module
        | RustSyntaxKind::Function
        | RustSyntaxKind::Method
        | RustSyntaxKind::Type
        | RustSyntaxKind::Trait
        | RustSyntaxKind::Impl
        | RustSyntaxKind::Constant
        | RustSyntaxKind::Static
        | RustSyntaxKind::MacroDefinition => EvidenceRole::Definition,
        RustSyntaxKind::Unknown => EvidenceRole::Reference,
    }
}

/// Extracts bounded structural facts from a tolerant syntax tree.
pub fn extract_structural_facts(
    tree: &RustSyntaxTree,
    representation: &RustRepresentation,
    profile: &QualifiedRustParserProfile,
    budget: EnrichmentBudget,
    cancellation: &dyn EnrichmentCancellation,
    blake3_256: impl Fn(&[u8]) -> [u8; 32] + Copy,
) -> Result<BoundedList<StructuralFact, MAX_LIST_ITEMS>, EnrichError> {
    let budget = budget.validate()?;
    validate_tree_identity(tree, representation, profile)?;
    let mut facts = Vec::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        if cancellation.is_cancelled() {
            return Err(EnrichError::ParseCancelled);
        }
        if facts.len() >= budget.max_facts {
            return Err(EnrichError::ParseBudgetExhausted);
        }
        node.range.validate(representation.bytes.len())?;
        let configuration = node
            .attributes
            .iter()
            .find_map(|attribute| {
                extract_configuration_predicate(
                    attribute,
                    node.range,
                    budget.max_configuration_depth,
                    blake3_256,
                )
                .transpose()
            })
            .transpose()?;
        let mut gaps = Vec::new();
        if node.recovered {
            gaps.push(EnrichmentGap::MalformedSource);
        }
        if configuration.as_ref().is_some_and(|value| !value.complete) {
            gaps.push(EnrichmentGap::ConfigurationUnknown);
        }
        let entity_kind = entity_kind(node.kind);
        let role = classify_evidence_role(node);
        let unit_id = if representation.unit_ids.is_empty() {
            None
        } else {
            representation
                .unit_ids
                .as_slice()
                .get(index.min(representation.unit_ids.len().saturating_sub(1)))
                .copied()
        };
        let digest_input = fact_digest_input(
            tree,
            node,
            entity_kind,
            role,
            configuration.as_ref(),
        )?;
        facts.push(StructuralFact {
            fact_digest: Blake3Digest32::from_bytes(blake3_256(&digest_input)),
            source_revision_ref: tree.source_revision_ref,
            representation_id: tree.representation_id,
            unit_id,
            entity_kind,
            name: node.name.clone(),
            range: node.range,
            evidence_role: role,
            configuration,
            parser_profile_digest: tree.parser_profile_digest,
            assurance: AssuranceClass::DescriptiveOnly,
            gaps: BoundedList::new(gaps).map_err(|_| EnrichError::ContractViolation)?,
        });
    }
    BoundedList::new(facts).map_err(|_| EnrichError::ContractViolation)
}

fn validate_tree_identity(
    tree: &RustSyntaxTree,
    representation: &RustRepresentation,
    profile: &QualifiedRustParserProfile,
) -> Result<(), EnrichError> {
    if tree.source_revision_ref != representation.source_revision_ref
        || tree.representation_id != representation.representation_id
        || tree.coordinate_map_digest != representation.coordinate_map_digest
    {
        return Err(EnrichError::AnchorMappingFailed);
    }
    if tree.parser_profile_digest != profile.profile.profile_digest {
        return Err(EnrichError::EnrichmentProfileMismatch);
    }
    Ok(())
}

fn fact_digest_input(
    tree: &RustSyntaxTree,
    node: &RustSyntaxNode,
    entity_kind: EntityKind,
    role: EvidenceRole,
    configuration: Option<&ConfigurationObservation>,
) -> Result<Vec<u8>, EnrichError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/structural-fact/v1")?;
    bytes.extend_from_slice(tree.source_revision_ref.revision_id.as_bytes());
    bytes.extend_from_slice(tree.representation_id.as_bytes());
    bytes.extend_from_slice(tree.parser_profile_digest.as_bytes());
    bytes.extend_from_slice(node.node_digest.as_bytes());
    append(&mut bytes, entity_kind.as_str().as_bytes())?;
    append(&mut bytes, role.as_str().as_bytes())?;
    if let Some(configuration) = configuration {
        bytes.extend_from_slice(configuration.predicate_digest.as_bytes());
    }
    Ok(bytes)
}

/// Descriptive, non-compiler relation class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StructuralRelationKind {
    /// Lexical nesting/containment.
    Contains,
    /// A declaration introduces a name.
    Declares,
    /// Impl header syntactically names a trait/type.
    ImplementsSyntactically,
    /// Invocation syntax names a callable or macro.
    InvokesSyntactically,
    /// Name appears as a reference but is not compiler-resolved.
    ReferencesName,
    /// Test syntax names or contains a subject.
    TestsSubject,
    /// Documentation is associated with a definition.
    DocumentsSubject,
    /// Fact is guarded by a configuration predicate.
    GuardedByConfiguration,
}

/// Bounded descriptive structural relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralRelation {
    /// Stable relation digest.
    pub relation_digest: Blake3Digest32,
    /// Source fact digest.
    pub from_fact_digest: Blake3Digest32,
    /// Exact target fact when structurally proven inside the representation.
    pub to_fact_digest: Option<Blake3Digest32>,
    /// Name-only unresolved target when exact identity is unavailable.
    pub unresolved_target_name: Option<String>,
    /// Relation class.
    pub kind: StructuralRelationKind,
    /// Whether target binding remains ambiguous.
    pub ambiguous: bool,
    /// Parser profile digest.
    pub parser_profile_digest: Blake3Digest32,
}

/// Extracts deterministic descriptive relations without name-based overbinding.
pub fn extract_structural_relations(
    facts: &BoundedList<StructuralFact, MAX_LIST_ITEMS>,
    profile: &QualifiedRustParserProfile,
    budget: EnrichmentBudget,
    cancellation: &dyn EnrichmentCancellation,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<BoundedList<StructuralRelation, MAX_LIST_ITEMS>, EnrichError> {
    let budget = budget.validate()?;
    if facts
        .iter()
        .any(|fact| fact.parser_profile_digest != profile.profile.profile_digest)
    {
        return Err(EnrichError::EnrichmentProfileMismatch);
    }
    let mut relations = Vec::new();
    let mut definitions_by_name: BTreeMap<&str, Vec<&StructuralFact>> = BTreeMap::new();
    for fact in facts {
        if fact.evidence_role == EvidenceRole::Definition {
            if let Some(name) = fact.name.as_deref() {
                definitions_by_name.entry(name).or_default().push(fact);
            }
        }
    }
    for fact in facts {
        if cancellation.is_cancelled() {
            return Err(EnrichError::ParseCancelled);
        }
        let mut pending = Vec::new();
        if fact.evidence_role == EvidenceRole::Definition {
            pending.push((StructuralRelationKind::Declares, None, None, false));
        }
        if fact.configuration.is_some() {
            pending.push((
                StructuralRelationKind::GuardedByConfiguration,
                None,
                None,
                false,
            ));
        }
        if fact.evidence_role == EvidenceRole::Test {
            pending.push((StructuralRelationKind::TestsSubject, None, None, true));
        }
        if fact.name.is_some() && fact.evidence_role == EvidenceRole::Reference {
            let name = fact.name.as_deref().expect("checked");
            match definitions_by_name.get(name).map(Vec::as_slice) {
                Some([target]) => pending.push((
                    StructuralRelationKind::ReferencesName,
                    Some(target.fact_digest),
                    None,
                    false,
                )),
                _ => pending.push((
                    StructuralRelationKind::ReferencesName,
                    None,
                    Some(name.to_owned()),
                    true,
                )),
            }
        }
        if fact.evidence_role == EvidenceRole::Definition && fact.name.is_some() {
            if let Some(parent) = nearest_parent(fact, facts) {
                pending.push((
                    StructuralRelationKind::Contains,
                    Some(fact.fact_digest),
                    parent.name.clone(),
                    false,
                ));
            }
        }
        for (kind, target, unresolved, ambiguous) in pending {
            if relations.len() >= budget.max_relations {
                return Err(EnrichError::ParseBudgetExhausted);
            }
            let digest_input = relation_digest_input(
                fact.fact_digest,
                target,
                unresolved.as_deref(),
                kind,
                ambiguous,
                profile.profile.profile_digest,
            )?;
            relations.push(StructuralRelation {
                relation_digest: Blake3Digest32::from_bytes(blake3_256(&digest_input)),
                from_fact_digest: fact.fact_digest,
                to_fact_digest: target,
                unresolved_target_name: unresolved,
                kind,
                ambiguous,
                parser_profile_digest: profile.profile.profile_digest,
            });
        }
    }
    relations.sort_by(|left, right| {
        left.from_fact_digest
            .cmp(&right.from_fact_digest)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.to_fact_digest.cmp(&right.to_fact_digest))
            .then_with(|| left.unresolved_target_name.cmp(&right.unresolved_target_name))
    });
    BoundedList::new(relations).map_err(|_| EnrichError::ContractViolation)
}

fn nearest_parent<'a>(
    fact: &StructuralFact,
    facts: &'a BoundedList<StructuralFact, MAX_LIST_ITEMS>,
) -> Option<&'a StructuralFact> {
    facts
        .iter()
        .filter(|candidate| {
            candidate.fact_digest != fact.fact_digest
                && candidate.range.byte_start <= fact.range.byte_start
                && candidate.range.byte_end >= fact.range.byte_end
        })
        .min_by_key(|candidate| candidate.range.byte_end - candidate.range.byte_start)
}

fn relation_digest_input(
    from: Blake3Digest32,
    to: Option<Blake3Digest32>,
    unresolved: Option<&str>,
    kind: StructuralRelationKind,
    ambiguous: bool,
    profile: Blake3Digest32,
) -> Result<Vec<u8>, EnrichError> {
    let mut bytes = Vec::new();
    append(&mut bytes, b"eliot-search/structural-relation/v1")?;
    bytes.extend_from_slice(from.as_bytes());
    match to {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(value.as_bytes());
        }
        None => bytes.push(0),
    }
    append(&mut bytes, unresolved.unwrap_or("").as_bytes())?;
    bytes.push(relation_kind_tag(kind));
    bytes.push(u8::from(ambiguous));
    bytes.extend_from_slice(profile.as_bytes());
    Ok(bytes)
}

/// Deterministic enrichment manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnrichmentManifest {
    /// Source revision.
    pub source_revision_ref: SourceRevisionRef,
    /// Representation identity.
    pub representation_id: RepresentationId,
    /// Parser profile digest.
    pub parser_profile_digest: Blake3Digest32,
    /// Fact count.
    pub fact_count: usize,
    /// Relation count.
    pub relation_count: usize,
    /// Diagnostic count.
    pub diagnostic_count: usize,
    /// Overall assurance.
    pub assurance: ProviderAssurance,
    /// Explicit distinct gaps.
    pub gaps: BoundedList<EnrichmentGap, MAX_LIST_ITEMS>,
    /// Digest of exact fact identities in canonical order.
    pub fact_set_digest: Blake3Digest32,
    /// Digest of exact relation identities in canonical order.
    pub relation_set_digest: Blake3Digest32,
    /// Digest of complete manifest fields.
    pub manifest_digest: Blake3Digest32,
    /// Source-backed enrichment receipt.
    pub receipt_ref: ReceiptRef,
}

/// Builds a bounded deterministic enrichment manifest.
pub fn build_enrichment_manifest(
    tree: &RustSyntaxTree,
    facts: &BoundedList<StructuralFact, MAX_LIST_ITEMS>,
    relations: &BoundedList<StructuralRelation, MAX_LIST_ITEMS>,
    receipt_ref: ReceiptRef,
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<EnrichmentManifest, EnrichError> {
    if facts.iter().any(|fact| {
        fact.source_revision_ref != tree.source_revision_ref
            || fact.representation_id != tree.representation_id
            || fact.parser_profile_digest != tree.parser_profile_digest
    }) || relations
        .iter()
        .any(|relation| relation.parser_profile_digest != tree.parser_profile_digest)
    {
        return Err(EnrichError::EnrichmentManifestInvalid);
    }
    let mut fact_digests = facts
        .iter()
        .map(|fact| fact.fact_digest)
        .collect::<Vec<_>>();
    fact_digests.sort_unstable();
    let mut relation_digests = relations
        .iter()
        .map(|relation| relation.relation_digest)
        .collect::<Vec<_>>();
    relation_digests.sort_unstable();
    let fact_set_digest = digest_list(
        b"eliot-search/structural-fact-set/v1",
        &fact_digests,
        blake3_256,
    )?;
    let relation_set_digest = digest_list(
        b"eliot-search/structural-relation-set/v1",
        &relation_digests,
        blake3_256,
    )?;
    let mut gaps = BTreeSet::new();
    if tree.state == ParseState::DegradedTolerantSyntax {
        gaps.insert(EnrichmentGap::MalformedSource);
    }
    for fact in facts {
        gaps.extend(fact.gaps.iter().copied());
    }
    if relations.iter().any(|relation| relation.ambiguous) {
        gaps.insert(EnrichmentGap::AmbiguousRelationTarget);
    }
    let assurance = assurance_for(tree.state);
    let mut manifest_input = Vec::new();
    append(&mut manifest_input, b"eliot-search/enrichment-manifest/v1")?;
    manifest_input.extend_from_slice(tree.source_revision_ref.revision_id.as_bytes());
    manifest_input.extend_from_slice(tree.representation_id.as_bytes());
    manifest_input.extend_from_slice(tree.parser_profile_digest.as_bytes());
    manifest_input.extend_from_slice(fact_set_digest.as_bytes());
    manifest_input.extend_from_slice(relation_set_digest.as_bytes());
    manifest_input.extend_from_slice(
        &u64::try_from(facts.len()).unwrap_or(u64::MAX).to_be_bytes(),
    );
    manifest_input.extend_from_slice(
        &u64::try_from(relations.len()).unwrap_or(u64::MAX).to_be_bytes(),
    );
    manifest_input.extend_from_slice(
        &u64::try_from(tree.diagnostics.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    manifest_input.push(assurance_tag(assurance));
    for gap in &gaps {
        manifest_input.push(gap_tag(*gap));
    }
    append(&mut manifest_input, receipt_ref.as_str().as_bytes())?;
    Ok(EnrichmentManifest {
        source_revision_ref: tree.source_revision_ref,
        representation_id: tree.representation_id,
        parser_profile_digest: tree.parser_profile_digest,
        fact_count: facts.len(),
        relation_count: relations.len(),
        diagnostic_count: tree.diagnostics.len(),
        assurance,
        gaps: BoundedList::new(gaps.into_iter().collect())
            .map_err(|_| EnrichError::ContractViolation)?,
        fact_set_digest,
        relation_set_digest,
        manifest_digest: Blake3Digest32::from_bytes(blake3_256(&manifest_input)),
        receipt_ref,
    })
}

/// Complete enrichment product.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralFacts {
    /// Tolerant syntax tree.
    pub tree: RustSyntaxTree,
    /// Structural facts.
    pub facts: BoundedList<StructuralFact, MAX_LIST_ITEMS>,
    /// Descriptive relations.
    pub relations: BoundedList<StructuralRelation, MAX_LIST_ITEMS>,
    /// Deterministic manifest.
    pub manifest: EnrichmentManifest,
}

/// Runs the no-execute baseline end-to-end.
pub fn enrich_code(
    representation: &RustRepresentation,
    profile: &QualifiedRustParserProfile,
    budget: EnrichmentBudget,
    cancellation: &dyn EnrichmentCancellation,
    receipt_ref: ReceiptRef,
    blake3_256: impl Fn(&[u8]) -> [u8; 32] + Copy,
) -> Result<StructuralFacts, EnrichError> {
    let tree = parse_rust_no_execute(
        representation,
        profile,
        budget,
        cancellation,
        blake3_256,
    )?;
    let facts = extract_structural_facts(
        &tree,
        representation,
        profile,
        budget,
        cancellation,
        blake3_256,
    )?;
    let relations = extract_structural_relations(
        &facts,
        profile,
        budget,
        cancellation,
        blake3_256,
    )?;
    let manifest = build_enrichment_manifest(
        &tree,
        &facts,
        &relations,
        receipt_ref,
        blake3_256,
    )?;
    Ok(StructuralFacts {
        tree,
        facts,
        relations,
        manifest,
    })
}

/// Behavior-affecting parser profile change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnrichmentProfileChange {
    /// Profiles are byte-for-byte equivalent in behavior.
    Noop,
    /// Existing representations require re-enrichment and reprojection.
    ReEnrichAndReproject,
    /// Assurance/no-execute/evidence semantics changed and require a gate.
    GateRequired,
    /// Candidate profile is invalid or would permit execution.
    Reject,
}

/// Classifies a profile change without reinterpreting old facts in place.
#[must_use]
pub fn compare_profile_change(
    old: &QualifiedRustParserProfile,
    new: &RustParserProfile,
) -> EnrichmentProfileChange {
    if !new.no_execute
        || new.supported_editions.is_empty()
        || new.max_input_bytes == 0
        || new.max_nodes == 0
        || new.max_depth == 0
    {
        return EnrichmentProfileChange::Reject;
    }
    if &old.profile == new {
        return EnrichmentProfileChange::Noop;
    }
    if old.profile.source_checksum != new.source_checksum
        || old.profile.parser_package != new.parser_package
        || old.profile.parser_version != new.parser_version
        || old.profile.golden_fixture_digest != new.golden_fixture_digest
        || old.profile.no_execute != new.no_execute
    {
        return EnrichmentProfileChange::GateRequired;
    }
    EnrichmentProfileChange::ReEnrichAndReproject
}

fn entity_kind(kind: RustSyntaxKind) -> EntityKind {
    match kind {
        RustSyntaxKind::Module => EntityKind::Module,
        RustSyntaxKind::Function => EntityKind::Function,
        RustSyntaxKind::Method => EntityKind::Method,
        RustSyntaxKind::Type => EntityKind::Type,
        RustSyntaxKind::Trait => EntityKind::Trait,
        RustSyntaxKind::Impl => EntityKind::Impl,
        RustSyntaxKind::Constant => EntityKind::Constant,
        RustSyntaxKind::Static => EntityKind::Static,
        RustSyntaxKind::MacroDefinition | RustSyntaxKind::MacroInvocation => EntityKind::Macro,
        RustSyntaxKind::Test => EntityKind::Test,
        RustSyntaxKind::Documentation => EntityKind::Document,
        RustSyntaxKind::Configuration => EntityKind::Unknown,
        RustSyntaxKind::Unknown => EntityKind::Unknown,
    }
}

fn digest_list(
    domain: &[u8],
    values: &[Blake3Digest32],
    blake3_256: impl Fn(&[u8]) -> [u8; 32],
) -> Result<Blake3Digest32, EnrichError> {
    let mut bytes = Vec::new();
    append(&mut bytes, domain)?;
    for value in values {
        bytes.extend_from_slice(value.as_bytes());
    }
    Ok(Blake3Digest32::from_bytes(blake3_256(&bytes)))
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> Result<(), EnrichError> {
    let length = u64::try_from(value.len()).map_err(|_| EnrichError::ParseBudgetExhausted)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    if output.len() > 8 * 1024 * 1024 {
        return Err(EnrichError::ParseBudgetExhausted);
    }
    Ok(())
}

const fn syntax_kind_tag(value: RustSyntaxKind) -> u8 {
    match value {
        RustSyntaxKind::Module => 1,
        RustSyntaxKind::Function => 2,
        RustSyntaxKind::Method => 3,
        RustSyntaxKind::Type => 4,
        RustSyntaxKind::Trait => 5,
        RustSyntaxKind::Impl => 6,
        RustSyntaxKind::Constant => 7,
        RustSyntaxKind::Static => 8,
        RustSyntaxKind::MacroDefinition => 9,
        RustSyntaxKind::MacroInvocation => 10,
        RustSyntaxKind::Test => 11,
        RustSyntaxKind::Documentation => 12,
        RustSyntaxKind::Configuration => 13,
        RustSyntaxKind::Unknown => 14,
    }
}

const fn relation_kind_tag(value: StructuralRelationKind) -> u8 {
    match value {
        StructuralRelationKind::Contains => 1,
        StructuralRelationKind::Declares => 2,
        StructuralRelationKind::ImplementsSyntactically => 3,
        StructuralRelationKind::InvokesSyntactically => 4,
        StructuralRelationKind::ReferencesName => 5,
        StructuralRelationKind::TestsSubject => 6,
        StructuralRelationKind::DocumentsSubject => 7,
        StructuralRelationKind::GuardedByConfiguration => 8,
    }
}

const fn assurance_tag(value: ProviderAssurance) -> u8 {
    match value {
        ProviderAssurance::TolerantSyntax => 1,
        ProviderAssurance::DegradedTolerantSyntax => 2,
        ProviderAssurance::Unavailable => 3,
    }
}

const fn gap_tag(value: EnrichmentGap) -> u8 {
    match value {
        EnrichmentGap::MalformedSource => 1,
        EnrichmentGap::ConfigurationUnknown => 2,
        EnrichmentGap::AmbiguousRelationTarget => 3,
        EnrichmentGap::BudgetOmission => 4,
        EnrichmentGap::AnchorMappingGap => 5,
    }
}

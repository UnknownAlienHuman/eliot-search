//! T35 code-enrichment qualification scenarios over exact retained units.
//!
//! The suite pins the no-execute tolerant baseline: golden facts stay
//! deterministic and byte-anchored, Unicode/CRLF offsets read back exactly,
//! macros are never expanded, and malformed/cancelled/budget-limited runs
//! degrade explicitly without a second symbol database or compiler claims.

// The golden corpus table is data, not logic; it lives in one function so the
// pinned (name, kind, role, entity) multiset stays reviewable in one place.
#![allow(clippy::too_many_lines)]

use search_code_enricher::{
    AnchorValidationReceipt, ConfigurationPredicate, EnrichError, EnrichmentBudget,
    EnrichmentCancellation, EnrichmentGap, EnrichmentProfileChange, MalformedInputPolicy,
    NeverCancelled, ParseState, ParserQualificationReceipt, ProviderAssurance,
    QualifiedRustParserProfile, RustEdition, RustParserProfile, RustRepresentation, RustSyntaxKind,
    StructuralFact, assurance_for, compare_profile_change, enrich_code,
    extract_configuration_predicate, extract_structural_facts, parse_rust_no_execute,
    validate_fact_anchor, validate_parser_profile,
};
use search_contracts::{
    AssuranceClass, Blake3Digest32, BoundedList, EntityKind, EvidenceRole, ProfileId, ReceiptRef,
    RepresentationId, SourceId, SourceNamespaceId, SourceRevisionId, SourceRevisionRef, UnitId,
};
use std::cell::Cell;
use std::collections::BTreeSet;

fn blake3_256(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0x5A_u8; 32];
    for (index, byte) in bytes.iter().enumerate() {
        let slot = index % 32;
        out[slot] = out[slot]
            .wrapping_add(*byte)
            .wrapping_add(out[(slot + 7) % 32]);
    }
    out
}

fn test_profile() -> RustParserProfile {
    RustParserProfile {
        profile_id: ProfileId::new("t35-baseline-profile").expect("profile id"),
        parser_package: ProfileId::new("eliot-search-builtin-tolerant-baseline")
            .expect("parser package"),
        parser_version: ProfileId::new("0.0.0-baseline.1").expect("parser version"),
        source_checksum: Blake3Digest32::from_bytes([1; 32]),
        license_receipt_ref: ReceiptRef::new("receipt:license-mit").expect("license receipt"),
        node_schema_digest: Blake3Digest32::from_bytes([2; 32]),
        query_schema_digest: Blake3Digest32::from_bytes([3; 32]),
        golden_fixture_digest: Blake3Digest32::from_bytes([4; 32]),
        supported_editions: BTreeSet::from([RustEdition::Rust2021, RustEdition::Rust2024]),
        max_input_bytes: 65_536,
        max_nodes: 256,
        max_depth: 64,
        max_attributes_per_node: 16,
        max_diagnostics: 64,
        malformed_policy: MalformedInputPolicy::RecoverWithGaps,
        no_execute: true,
        profile_digest: Blake3Digest32::from_bytes([7; 32]),
    }
}

fn qualification_for(profile: &RustParserProfile) -> ParserQualificationReceipt {
    ParserQualificationReceipt {
        profile_digest: profile.profile_digest,
        source_checksum: profile.source_checksum,
        golden_fixture_digest: profile.golden_fixture_digest,
        no_execute_audit_receipt_ref: ReceiptRef::new("receipt:no-execute-audit")
            .expect("audit receipt"),
        qualification_receipt_ref: ReceiptRef::new("receipt:parser-qualification")
            .expect("qualification receipt"),
    }
}

fn qualified_profile() -> QualifiedRustParserProfile {
    let profile = test_profile();
    let receipt = qualification_for(&profile);
    validate_parser_profile(profile, receipt).expect("qualified profile")
}

fn representation_of(text: &str, edition: RustEdition, units: Vec<UnitId>) -> RustRepresentation {
    let bytes = text.as_bytes().to_vec();
    let byte_length = u64::try_from(bytes.len()).expect("byte length fits u64");
    RustRepresentation {
        source_revision_ref: SourceRevisionRef {
            source_namespace_id: SourceNamespaceId::from_bytes([0x11; 16]),
            source_id: SourceId::from_bytes([0x22; 16]),
            revision_id: SourceRevisionId::from_bytes([0x33; 16]),
            content_digest: Blake3Digest32::from_bytes([0x44; 32]),
            byte_length,
        },
        representation_id: RepresentationId::from_bytes([0x55; 16]),
        coordinate_map_digest: Blake3Digest32::from_bytes([0x66; 32]),
        representation_digest: Blake3Digest32::from_bytes([0x77; 32]),
        edition,
        bytes,
        unit_ids: BoundedList::new(units).expect("unit ids fit bounds"),
    }
}

fn retained_units() -> Vec<UnitId> {
    vec![
        UnitId::from_bytes([0xA1; 16]),
        UnitId::from_bytes([0xA2; 16]),
    ]
}

const fn budget() -> EnrichmentBudget {
    EnrichmentBudget {
        max_input_bytes: 65_536,
        max_lines: 4_096,
        max_nodes: 256,
        max_facts: 256,
        max_relations: 512,
        max_configuration_depth: 16,
        max_steps: 1_000_000,
    }
}

const GOLDEN_RUST: &str = concat!(
    "/// Adds two numbers.\n",
    "pub fn add(left: u64, right: u64) -> u64 {\n",
    "    left + right\n",
    "}\n",
    "\n",
    "/// A point.\n",
    "pub struct Point {\n",
    "    pub x: f64,\n",
    "}\n",
    "\n",
    "pub enum Direction {\n",
    "    North,\n",
    "}\n",
    "\n",
    "pub trait Shape {\n",
    "    fn area(&self) -> f64;\n",
    "}\n",
    "\n",
    "pub struct Circle {\n",
    "    pub radius: f64,\n",
    "}\n",
    "\n",
    "impl Shape for Circle {\n",
    "    fn area(&self) -> f64 {\n",
    "        3.0\n",
    "    }\n",
    "}\n",
    "\n",
    "impl Circle {\n",
    "    pub fn new(radius: f64) -> Self {\n",
    "        Circle { radius }\n",
    "    }\n",
    "}\n",
    "\n",
    "pub const MAX_POINTS: usize = 1024;\n",
    "pub static VERSION: &str = \"0.0.0\";\n",
    "\n",
    "macro_rules! define_id {\n",
    "    ($name:ident) => {};\n",
    "}\n",
    "\n",
    "define_id!(SessionId);\n",
    "\n",
    "#[cfg(any(windows, target_os = \"linux\"))]\n",
    "pub fn platform_only() {}\n",
    "\n",
    "#[test]\n",
    "fn adds_numbers() {\n",
    "    assert_eq!(add(1, 2), 3);\n",
    "    println!(\"done\");\n",
    "}\n",
    "\n",
    "#[cfg(test)]\n",
    "mod unit_tests {\n",
    "    #[test]\n",
    "    fn adds_again() {}\n",
    "}\n",
    "\n",
    "pub mod nested {\n",
    "    pub fn inner() {}\n",
    "}\n",
);

fn golden_facts() -> Vec<StructuralFact> {
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-golden").expect("receipt"),
        blake3_256,
    )
    .expect("golden corpus enriches");
    enriched.facts.into_vec()
}

fn fact_named(facts: &[StructuralFact], name: &str) -> StructuralFact {
    facts
        .iter()
        .find(|fact| fact.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("expected fact named {name}"))
        .clone()
}

#[test]
fn golden_corpus_fact_count_is_pinned() {
    assert_eq!(golden_facts().len(), 22);
}

#[test]
fn golden_corpus_kinds_and_roles_are_pinned() {
    let facts = golden_facts();
    let mut observed: Vec<(Option<String>, RustSyntaxKind, EvidenceRole, EntityKind)> = facts
        .iter()
        .map(|fact| {
            (
                fact.name.clone(),
                syntax_kind_of(fact),
                fact.evidence_role,
                fact.entity_kind,
            )
        })
        .collect();
    observed.sort();
    let owned = |name: &str| Some(name.to_owned());
    let mut expected = vec![
        (
            owned("MAX_POINTS"),
            RustSyntaxKind::Constant,
            EvidenceRole::Definition,
            EntityKind::Constant,
        ),
        (
            owned("Circle"),
            RustSyntaxKind::Impl,
            EvidenceRole::Definition,
            EntityKind::Impl,
        ),
        (
            owned("Circle"),
            RustSyntaxKind::Type,
            EvidenceRole::Definition,
            EntityKind::Type,
        ),
        (
            owned("Direction"),
            RustSyntaxKind::Type,
            EvidenceRole::Definition,
            EntityKind::Type,
        ),
        (
            owned("Point"),
            RustSyntaxKind::Type,
            EvidenceRole::Definition,
            EntityKind::Type,
        ),
        (
            owned("Shape"),
            RustSyntaxKind::Impl,
            EvidenceRole::Definition,
            EntityKind::Impl,
        ),
        (
            owned("Shape"),
            RustSyntaxKind::Trait,
            EvidenceRole::Definition,
            EntityKind::Trait,
        ),
        (
            owned("VERSION"),
            RustSyntaxKind::Static,
            EvidenceRole::Definition,
            EntityKind::Static,
        ),
        (
            owned("add"),
            RustSyntaxKind::Function,
            EvidenceRole::Definition,
            EntityKind::Function,
        ),
        (
            owned("adds_again"),
            RustSyntaxKind::Test,
            EvidenceRole::Test,
            EntityKind::Test,
        ),
        (
            owned("adds_numbers"),
            RustSyntaxKind::Test,
            EvidenceRole::Test,
            EntityKind::Test,
        ),
        (
            owned("area"),
            RustSyntaxKind::Method,
            EvidenceRole::Definition,
            EntityKind::Method,
        ),
        (
            owned("area"),
            RustSyntaxKind::Method,
            EvidenceRole::Definition,
            EntityKind::Method,
        ),
        (
            owned("assert_eq"),
            RustSyntaxKind::MacroInvocation,
            EvidenceRole::Reference,
            EntityKind::Macro,
        ),
        (
            owned("define_id"),
            RustSyntaxKind::MacroDefinition,
            EvidenceRole::Definition,
            EntityKind::Macro,
        ),
        (
            owned("define_id"),
            RustSyntaxKind::MacroInvocation,
            EvidenceRole::Reference,
            EntityKind::Macro,
        ),
        (
            owned("inner"),
            RustSyntaxKind::Method,
            EvidenceRole::Definition,
            EntityKind::Method,
        ),
        (
            owned("nested"),
            RustSyntaxKind::Module,
            EvidenceRole::Definition,
            EntityKind::Module,
        ),
        (
            owned("new"),
            RustSyntaxKind::Method,
            EvidenceRole::Definition,
            EntityKind::Method,
        ),
        (
            owned("platform_only"),
            RustSyntaxKind::Function,
            EvidenceRole::Definition,
            EntityKind::Function,
        ),
        (
            owned("println"),
            RustSyntaxKind::MacroInvocation,
            EvidenceRole::Reference,
            EntityKind::Macro,
        ),
        (
            owned("unit_tests"),
            RustSyntaxKind::Test,
            EvidenceRole::Test,
            EntityKind::Test,
        ),
    ];
    expected.sort();
    assert_eq!(observed, expected);
    for fact in &facts {
        assert_eq!(
            fact.assurance,
            AssuranceClass::DescriptiveOnly,
            "tolerant assurance never claims compiler truth"
        );
    }
}

/// Recovers the syntax kind through the closed evidence-role mapping plus
/// entity kind, since [`StructuralFact`] stores the contract vocabulary
/// rather than the vendor-neutral syntax kind.
fn syntax_kind_of(fact: &StructuralFact) -> RustSyntaxKind {
    match (fact.entity_kind, fact.evidence_role) {
        (EntityKind::Function, _) => RustSyntaxKind::Function,
        (EntityKind::Method, _) => RustSyntaxKind::Method,
        (EntityKind::Type, _) => RustSyntaxKind::Type,
        (EntityKind::Trait, _) => RustSyntaxKind::Trait,
        (EntityKind::Impl, _) => RustSyntaxKind::Impl,
        (EntityKind::Constant, _) => RustSyntaxKind::Constant,
        (EntityKind::Static, _) => RustSyntaxKind::Static,
        (EntityKind::Module, _) => RustSyntaxKind::Module,
        (EntityKind::Test, _) => RustSyntaxKind::Test,
        (EntityKind::Macro, EvidenceRole::Reference) => RustSyntaxKind::MacroInvocation,
        (EntityKind::Macro, _) => RustSyntaxKind::MacroDefinition,
        (unexpected, role) => panic!("unexpected golden fact kind: {unexpected:?}/{role:?}"),
    }
}

#[test]
fn golden_corpus_is_deterministic() {
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    let receipt = ReceiptRef::new("receipt:t35-determinism").expect("receipt");
    let first = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        receipt,
        blake3_256,
    )
    .expect("first enrichment");
    let receipt = ReceiptRef::new("receipt:t35-determinism").expect("receipt");
    let second = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        receipt,
        blake3_256,
    )
    .expect("second enrichment");
    assert_eq!(first, second);
    assert_eq!(first.manifest.fact_count, first.facts.len());
    assert_eq!(first.manifest.relation_count, first.relations.len());
}

#[test]
fn golden_relations_keep_ambiguous_targets_explicit() {
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-relations").expect("receipt"),
        blake3_256,
    )
    .expect("golden relations enrich");
    // 16 Declares + 2 GuardedByConfiguration (platform_only, unit_tests)
    // + 3 TestsSubject + 3 ReferencesName.
    assert_eq!(enriched.relations.len(), 24);
    let digest_set: Vec<_> = enriched.facts.iter().map(|fact| fact.fact_digest).collect();
    for relation in &enriched.relations {
        assert_eq!(
            relation.parser_profile_digest,
            profile.profile().profile_digest
        );
        if let Some(target) = relation.to_fact_digest {
            assert!(
                digest_set.contains(&target),
                "resolved targets stay inside the representation"
            );
            assert!(!relation.ambiguous);
        } else {
            assert!(
                relation.unresolved_target_name.is_some()
                    || relation.kind == search_code_enricher::StructuralRelationKind::Declares
                    || relation.kind
                        == search_code_enricher::StructuralRelationKind::GuardedByConfiguration
                    || relation.kind == search_code_enricher::StructuralRelationKind::TestsSubject,
                "every unbound relation names its target or is self-descriptive"
            );
        }
    }
    assert!(
        enriched
            .manifest
            .gaps
            .iter()
            .any(|gap| *gap == EnrichmentGap::AmbiguousRelationTarget),
        "println/assert_eq ambiguity is an explicit manifest gap"
    );
}

#[test]
fn every_golden_anchor_reads_back_exact_coordinates() {
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-anchors").expect("receipt"),
        blake3_256,
    )
    .expect("golden anchors enrich");
    assert!(!enriched.facts.is_empty());
    for fact in &enriched.facts {
        let receipt: AnchorValidationReceipt =
            validate_fact_anchor(fact, &representation).expect("anchor validates");
        assert_eq!(receipt.fact_digest, fact.fact_digest);
        assert_eq!(receipt.representation_id, representation.representation_id);
        assert_eq!(receipt.range, fact.range);
        assert_eq!(
            receipt.parser_profile_digest, fact.parser_profile_digest,
            "provenance binds the qualified profile"
        );
    }
}

#[test]
fn anchor_validation_rejects_foreign_representation() {
    let facts = golden_facts();
    let fact = facts.first().expect("golden fact");
    let mut foreign = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    foreign.representation_id = RepresentationId::from_bytes([0x99; 16]);
    assert_eq!(
        validate_fact_anchor(fact, &foreign),
        Err(EnrichError::AnchorMappingFailed)
    );
}

#[test]
fn anchor_validation_rejects_unmapped_range() {
    let facts = golden_facts();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    let mut fact = facts.first().expect("golden fact").clone();
    fact.range.byte_end = u64::try_from(representation.bytes.len() + 64).expect("end fits u64");
    assert_eq!(
        validate_fact_anchor(&fact, &representation),
        Err(EnrichError::StructuralFactUnmapped)
    );
}

#[test]
fn unicode_offsets_read_back_exact_bytes() {
    let text = concat!(
        "/// Функция соединения — naïve façade.\n",
        "pub fn connecter() -> bool {\n",
        "    true\n",
        "}\n",
        "\n",
        "fn привіт() {}\n",
    );
    let profile = qualified_profile();
    let representation = representation_of(text, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-unicode").expect("receipt"),
        blake3_256,
    )
    .expect("unicode corpus enriches");
    let connecter = fact_named(&enriched.facts.into_vec(), "connecter");
    let start = usize::try_from(connecter.range.byte_start).expect("start fits usize");
    let end = usize::try_from(connecter.range.byte_end).expect("end fits usize");
    assert_eq!(
        &representation.bytes[start..end],
        b"pub fn connecter() -> bool {"
    );
    validate_fact_anchor(&connecter, &representation).expect("unicode anchor validates");
}

#[test]
fn unicode_identifier_is_recovered_with_exact_anchor_not_truth() {
    let text = "fn привіт() {}\n";
    let profile = qualified_profile();
    let representation = representation_of(text, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-unicode-ident").expect("receipt"),
        blake3_256,
    )
    .expect("unicode identifier enriches tolerantly");
    assert_eq!(enriched.tree.state, ParseState::DegradedTolerantSyntax);
    let node = enriched.tree.nodes.iter().next().expect("one node");
    assert!(node.recovered, "non-ASCII identifier is explicit recovery");
    assert_eq!(node.name, None);
    let start = usize::try_from(node.range.byte_start).expect("start fits usize");
    let end = usize::try_from(node.range.byte_end).expect("end fits usize");
    assert_eq!(
        &representation.bytes[start..end],
        "fn привіт() {}".as_bytes()
    );
    for fact in &enriched.facts {
        assert_eq!(fact.assurance, AssuranceClass::DescriptiveOnly);
        validate_fact_anchor(fact, &representation).expect("recovered anchor validates");
    }
}

#[test]
fn crlf_offsets_map_to_exact_lines() {
    let text = "pub fn crlf() {}\r\npub struct CrlfStruct {}\r\n";
    let profile = qualified_profile();
    let representation = representation_of(text, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-crlf").expect("receipt"),
        blake3_256,
    )
    .expect("crlf corpus enriches");
    assert_eq!(enriched.facts.len(), 2);
    let first = &enriched.facts.as_slice()[0];
    let second = &enriched.facts.as_slice()[1];
    assert_eq!((first.range.line_start, first.range.line_end), (0, 1));
    assert_eq!((second.range.line_start, second.range.line_end), (1, 2));
    for (fact, expected) in [
        (first, "pub fn crlf() {}"),
        (second, "pub struct CrlfStruct {}"),
    ] {
        let start = usize::try_from(fact.range.byte_start).expect("start fits usize");
        let end = usize::try_from(fact.range.byte_end).expect("end fits usize");
        assert_eq!(&representation.bytes[start..end], expected.as_bytes());
        validate_fact_anchor(fact, &representation).expect("crlf anchor validates");
    }
}

#[test]
fn macros_are_observed_not_expanded() {
    let text = concat!(
        "macro_rules! emit {\n",
        "    ($body:expr) => { $body };\n",
        "}\n",
        "\n",
        "pub fn run() {\n",
        "    emit!(42);\n",
        "    include_str!(\"data.txt\");\n",
        "}\n",
    );
    let profile = qualified_profile();
    let representation = representation_of(text, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-macro").expect("receipt"),
        blake3_256,
    )
    .expect("macro corpus enriches");
    let definition = enriched
        .facts
        .iter()
        .find(|fact| {
            fact.name.as_deref() == Some("emit") && fact.evidence_role == EvidenceRole::Definition
        })
        .expect("macro definition fact");
    assert_eq!(definition.entity_kind, EntityKind::Macro);
    for name in ["emit", "include_str"] {
        let invocation = enriched
            .facts
            .iter()
            .find(|fact| {
                fact.name.as_deref() == Some(name) && fact.evidence_role == EvidenceRole::Reference
            })
            .unwrap_or_else(|| panic!("reference-only invocation fact for {name}"));
        assert_eq!(invocation.entity_kind, EntityKind::Macro);
        assert_eq!(invocation.assurance, AssuranceClass::DescriptiveOnly);
    }
    assert_eq!(
        enriched.facts.len(),
        4,
        "macro bodies contribute no expanded facts"
    );
}

#[test]
fn generated_marker_file_stays_bounded_and_descriptive() {
    let text = concat!(
        "// Code generated by eliotschemagen. DO NOT EDIT.\n",
        "\n",
        "#[automatically_derived]\n",
        "pub struct Generated {\n",
        "    pub field: u64,\n",
        "}\n",
    );
    let profile = qualified_profile();
    let representation = representation_of(text, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-generated").expect("receipt"),
        blake3_256,
    )
    .expect("generated file enriches");
    let generated = fact_named(&enriched.facts.into_vec(), "Generated");
    assert_eq!(generated.entity_kind, EntityKind::Type);
    assert_eq!(generated.assurance, AssuranceClass::DescriptiveOnly);
    validate_fact_anchor(&generated, &representation).expect("generated anchor validates");
}

#[test]
fn malformed_source_degrades_with_explicit_gap() {
    let text = "pub fn broken() {\n    let value = 1;\n";
    let profile = qualified_profile();
    let representation = representation_of(text, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-malformed").expect("receipt"),
        blake3_256,
    )
    .expect("malformed source recovers");
    assert_eq!(enriched.tree.state, ParseState::DegradedTolerantSyntax);
    assert_eq!(
        enriched.manifest.assurance,
        ProviderAssurance::DegradedTolerantSyntax
    );
    assert!(
        enriched
            .manifest
            .gaps
            .iter()
            .any(|gap| *gap == EnrichmentGap::MalformedSource)
    );
    for fact in &enriched.facts {
        assert_eq!(fact.assurance, AssuranceClass::DescriptiveOnly);
        validate_fact_anchor(fact, &representation).expect("degraded anchor validates");
    }
}

#[test]
fn reject_policy_refuses_malformed_source() {
    let mut profile = test_profile();
    profile.malformed_policy = MalformedInputPolicy::Reject;
    let receipt = qualification_for(&profile);
    let qualified = validate_parser_profile(profile, receipt).expect("reject profile qualifies");
    let representation = representation_of(
        "pub fn broken() {\n    let value = 1;\n",
        RustEdition::Rust2021,
        retained_units(),
    );
    assert_eq!(
        enrich_code(
            &representation,
            &qualified,
            budget(),
            &NeverCancelled,
            ReceiptRef::new("receipt:t35-reject").expect("receipt"),
            blake3_256,
        ),
        Err(EnrichError::ParseDegraded)
    );
}

#[test]
fn suspected_structural_line_becomes_bounded_recovery_gap() {
    let text = "let handler = fn pointer;\npub fn healthy() {}\n";
    let profile = qualified_profile();
    let representation = representation_of(text, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-recovery").expect("receipt"),
        blake3_256,
    )
    .expect("recovery enriches");
    assert_eq!(enriched.tree.state, ParseState::DegradedTolerantSyntax);
    assert!(enriched.tree.diagnostics.iter().any(
        |diagnostic| diagnostic.kind == search_code_enricher::ParseDiagnosticKind::RecoveryNode
    ));
    fact_named(&enriched.facts.into_vec(), "healthy");
}

struct CancelNow;

impl EnrichmentCancellation for CancelNow {
    fn is_cancelled(&self) -> bool {
        true
    }
}

struct CancelAfter {
    remaining: Cell<usize>,
}

impl EnrichmentCancellation for CancelAfter {
    fn is_cancelled(&self) -> bool {
        let left = self.remaining.get();
        if left == 0 {
            return true;
        }
        self.remaining.set(left - 1);
        false
    }
}

#[test]
fn immediate_cancel_yields_no_manifest() {
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    assert_eq!(
        enrich_code(
            &representation,
            &profile,
            budget(),
            &CancelNow,
            ReceiptRef::new("receipt:t35-cancel").expect("receipt"),
            blake3_256,
        ),
        Err(EnrichError::ParseCancelled)
    );
}

#[test]
fn mid_stream_cancel_yields_no_complete_facts() {
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    let tree = parse_rust_no_execute(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        blake3_256,
    )
    .expect("tree parses");
    let cancel = CancelAfter {
        remaining: Cell::new(1),
    };
    assert_eq!(
        extract_structural_facts(
            &tree,
            &representation,
            &profile,
            budget(),
            &cancel,
            blake3_256,
        ),
        Err(EnrichError::ParseCancelled)
    );
}

#[test]
fn exhausted_budgets_fail_closed_without_manifest() {
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    let receipt = ReceiptRef::new("receipt:t35-budget").expect("receipt");
    let mut tiny = budget();
    tiny.max_nodes = 1;
    assert_eq!(
        enrich_code(
            &representation,
            &profile,
            tiny,
            &NeverCancelled,
            receipt,
            blake3_256,
        ),
        Err(EnrichError::ParseBudgetExhausted)
    );
    let receipt = ReceiptRef::new("receipt:t35-budget").expect("receipt");
    let mut tiny = budget();
    tiny.max_facts = 1;
    assert_eq!(
        enrich_code(
            &representation,
            &profile,
            tiny,
            &NeverCancelled,
            receipt,
            blake3_256,
        ),
        Err(EnrichError::ParseBudgetExhausted)
    );
    let receipt = ReceiptRef::new("receipt:t35-budget").expect("receipt");
    let mut tiny = budget();
    tiny.max_steps = 1;
    assert_eq!(
        enrich_code(
            &representation,
            &profile,
            tiny,
            &NeverCancelled,
            receipt,
            blake3_256,
        ),
        Err(EnrichError::ParseBudgetExhausted)
    );
}

#[test]
fn cfg_variants_stay_distinguishable_and_unevaluated() {
    let range = search_code_enricher::TextRange {
        byte_start: 0,
        byte_end: 1,
        line_start: 0,
        line_end: 1,
    };
    let observation = extract_configuration_predicate(
        "#[cfg(all(unix, not(target_os = \"macos\")))]",
        range,
        16,
        blake3_256,
    )
    .expect("cfg parses")
    .expect("cfg observed");
    assert!(observation.complete);
    assert!(matches!(
        observation.predicate,
        ConfigurationPredicate::All(_)
    ));

    let observation = extract_configuration_predicate(
        "#[cfg(any(windows, target_os = \"linux\"))]",
        range,
        16,
        blake3_256,
    )
    .expect("cfg parses")
    .expect("cfg observed");
    assert!(observation.complete);
    assert!(matches!(
        observation.predicate,
        ConfigurationPredicate::Any(_)
    ));

    let observation = extract_configuration_predicate("#[cfg(not(test))]", range, 16, blake3_256)
        .expect("cfg parses")
        .expect("cfg observed");
    assert!(matches!(
        observation.predicate,
        ConfigurationPredicate::Not(_)
    ));

    let observation = extract_configuration_predicate("#[cfg(test)]", range, 16, blake3_256)
        .expect("cfg parses")
        .expect("cfg observed");
    assert_eq!(
        observation.predicate,
        ConfigurationPredicate::Key("test".to_owned())
    );

    let observation =
        extract_configuration_predicate("#[cfg(target_os = \"windows\")]", range, 16, blake3_256)
            .expect("cfg parses")
            .expect("cfg observed");
    assert_eq!(
        observation.predicate,
        ConfigurationPredicate::KeyValue {
            key: "target_os".to_owned(),
            value: "windows".to_owned(),
        }
    );

    let observation = extract_configuration_predicate("#[cfg(foo(bar))]", range, 16, blake3_256)
        .expect("unknown cfg parses")
        .expect("unknown cfg observed");
    assert!(!observation.complete);
    assert_eq!(observation.predicate, ConfigurationPredicate::Unknown);

    assert!(
        extract_configuration_predicate("#[derive(Debug)]", range, 16, blake3_256)
            .expect("non-cfg parses")
            .is_none()
    );
    // A truncated predicate degrades to explicit unknown, never to a silent
    // unconditional claim.
    let observation = extract_configuration_predicate("#[cfg(all(unix)]", range, 16, blake3_256)
        .expect("truncated cfg parses tolerantly")
        .expect("truncated cfg observed");
    assert!(!observation.complete);
    assert_eq!(observation.predicate, ConfigurationPredicate::Unknown);
    // Structurally unbalanced nesting across a closed outer form is rejected.
    assert_eq!(
        extract_configuration_predicate("#[cfg(all(unix), (windows))]", range, 16, blake3_256),
        Err(EnrichError::ConfigurationAmbiguous)
    );
}

#[test]
fn cfg_guarded_fact_links_predicate_without_claiming_inclusion() {
    let facts = golden_facts();
    let guarded = fact_named(&facts, "platform_only");
    let configuration = guarded.configuration.expect("cfg predicate retained");
    assert!(configuration.complete);
    assert!(matches!(
        configuration.predicate,
        ConfigurationPredicate::Any(_)
    ));
}

#[test]
fn floating_or_unqualified_profiles_are_rejected() {
    let mut floating = test_profile();
    floating.parser_version = ProfileId::new("latest").expect("floating version");
    let receipt = qualification_for(&floating);
    assert_eq!(
        validate_parser_profile(floating, receipt),
        Err(EnrichError::ParserProfileInvalid)
    );

    let mut unexecutable = test_profile();
    unexecutable.no_execute = false;
    let receipt = qualification_for(&unexecutable);
    assert_eq!(
        validate_parser_profile(unexecutable, receipt),
        Err(EnrichError::ParserProfileInvalid)
    );

    let mut empty_editions = test_profile();
    empty_editions.supported_editions = BTreeSet::new();
    let receipt = qualification_for(&empty_editions);
    assert_eq!(
        validate_parser_profile(empty_editions, receipt),
        Err(EnrichError::ParserProfileInvalid)
    );

    let profile = test_profile();
    let mut mismatched = qualification_for(&profile);
    mismatched.profile_digest = Blake3Digest32::from_bytes([0xFF; 32]);
    assert_eq!(
        validate_parser_profile(profile, mismatched),
        Err(EnrichError::ParserNotQualified)
    );
}

#[test]
fn unsupported_editions_and_oversize_inputs_are_rejected() {
    let mut profile = test_profile();
    profile.supported_editions = BTreeSet::from([RustEdition::Rust2021]);
    let receipt = qualification_for(&profile);
    let qualified = validate_parser_profile(profile, receipt).expect("edition profile qualifies");
    let representation =
        representation_of("pub fn old() {}\n", RustEdition::Rust2015, retained_units());
    assert_eq!(
        enrich_code(
            &representation,
            &qualified,
            budget(),
            &NeverCancelled,
            ReceiptRef::new("receipt:t35-edition").expect("receipt"),
            blake3_256,
        ),
        Err(EnrichError::RustInputUnsupported)
    );
    assert!(
        enrich_code(
            &representation_of("�", RustEdition::Rust2021, retained_units()),
            &qualified_profile(),
            budget(),
            &NeverCancelled,
            ReceiptRef::new("receipt:t35-replacement").expect("receipt"),
            blake3_256,
        )
        .is_ok(),
        "replacement character is valid UTF-8 and stays bounded"
    );

    let mut tiny_profile = test_profile();
    tiny_profile.max_input_bytes = 4;
    let receipt = qualification_for(&tiny_profile);
    let tiny = validate_parser_profile(tiny_profile, receipt).expect("tiny profile qualifies");
    assert_eq!(
        enrich_code(
            &representation_of("pub fn old() {}\n", RustEdition::Rust2021, retained_units()),
            &tiny,
            budget(),
            &NeverCancelled,
            ReceiptRef::new("receipt:t35-size").expect("receipt"),
            blake3_256,
        ),
        Err(EnrichError::RustInputTooLarge)
    );
}

#[test]
fn non_utf8_input_is_rejected_explicitly() {
    let profile = qualified_profile();
    let mut representation =
        representation_of("pub fn ok() {}\n", RustEdition::Rust2021, retained_units());
    representation.bytes = vec![0x66, 0x6E, 0x20, 0xFF, 0xFE, 0x0A];
    representation.source_revision_ref.byte_length =
        u64::try_from(representation.bytes.len()).expect("byte length fits u64");
    assert_eq!(
        enrich_code(
            &representation,
            &profile,
            budget(),
            &NeverCancelled,
            ReceiptRef::new("receipt:t35-nonutf8").expect("receipt"),
            blake3_256,
        ),
        Err(EnrichError::RustInputUnsupported)
    );
}

#[test]
fn profile_changes_classify_reenrichment_and_gates() {
    let qualified = qualified_profile();
    assert_eq!(
        compare_profile_change(&qualified, qualified.profile()),
        EnrichmentProfileChange::Noop
    );
    let mut relimited = qualified.profile().clone();
    relimited.max_nodes = 64;
    assert_eq!(
        compare_profile_change(&qualified, &relimited),
        EnrichmentProfileChange::ReEnrichAndReproject
    );
    let mut reversioned = qualified.profile().clone();
    reversioned.parser_version = ProfileId::new("0.0.0-baseline.2").expect("next version");
    assert_eq!(
        compare_profile_change(&qualified, &reversioned),
        EnrichmentProfileChange::GateRequired
    );
    let mut executing = qualified.profile().clone();
    executing.no_execute = false;
    assert_eq!(
        compare_profile_change(&qualified, &executing),
        EnrichmentProfileChange::Reject
    );
}

#[test]
fn facts_use_only_existing_projection_vocabulary() {
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, retained_units());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-projection").expect("receipt"),
        blake3_256,
    )
    .expect("projection vocabulary enriches");
    assert!(!enriched.facts.is_empty());
    for fact in &enriched.facts {
        assert!(
            EntityKind::ALL.contains(&fact.entity_kind),
            "fact kind is existing projection vocabulary"
        );
        assert!(
            search_contracts::EvidenceRole::ALL.contains(&fact.evidence_role),
            "fact role is existing projection vocabulary"
        );
        assert_eq!(
            fact.assurance,
            AssuranceClass::DescriptiveOnly,
            "facts never claim compiler certainty"
        );
        assert_eq!(fact.parser_profile_digest, profile.profile().profile_digest);
        assert_eq!(fact.source_revision_ref, representation.source_revision_ref);
        assert_eq!(fact.representation_id, representation.representation_id);
    }
    assert_eq!(
        enriched.manifest.assurance,
        assurance_for(enriched.tree.state)
    );
}

#[test]
fn retained_units_bind_facts_in_source_order() {
    let units = retained_units();
    let profile = qualified_profile();
    let representation = representation_of(GOLDEN_RUST, RustEdition::Rust2021, units.clone());
    let enriched = enrich_code(
        &representation,
        &profile,
        budget(),
        &NeverCancelled,
        ReceiptRef::new("receipt:t35-units").expect("receipt"),
        blake3_256,
    )
    .expect("unit-bound enrichment");
    for (index, fact) in enriched.facts.iter().enumerate() {
        let expected = units[index.min(units.len() - 1)];
        assert_eq!(
            fact.unit_id,
            Some(expected),
            "ordinal unit binding for fact {index}"
        );
    }
}

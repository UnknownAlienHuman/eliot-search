# `search-code-enricher` implementation packet

**Path:** `crates/search-prep/search-code-enricher`  
**Capability:** C10  
**Delivery:** W5 / P10  
**Gate:** BLOCKED until W5 is active and revision/materialization/unitization receipts are accepted  
**Trace:** S17.3, S21, P10  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Add a qualified Rust structural fact profile—definitions, references, tests, docs and configuration predicates—without claiming compiler truth.

## Owns

- Rust parser profile/version and qualification fixture
- structural fact extraction and relation manifests
- provider assurance labels
- cfg/configuration predicate propagation

## Must not own

- compiler certainty or type-checking claims
- running build scripts, proc macros or language-server build commands
- ranking/fusion
- supporting other languages without a separate profile boundary

## Logical primitives

- CodeEnrichmentRequest, ParserProfileDescriptor, StructuralFact, StructuralRelation, EvidenceRole, ConfigurationPredicate, ProviderAssurance, EnrichmentManifest

## Logical operations

1. `enrich_rust(representation, units, profile, budget) -> Result<EnrichmentManifest, EnrichError>`
2. `classify_evidence_role(node, context) -> EvidenceRole`
3. `extract_configuration_predicate(node) -> Option<ConfigurationPredicate>`
4. `validate_fact_anchor(fact, representation) -> Result<(), EnrichError>`

## Required invariants

- every fact carries parser profile and assurance
- malformed code may produce bounded tolerant facts but never compiler truth
- cfg variants remain distinct and explicit
- references/tests/docs are evidence roles, not truth
- parser never executes repository code

## Typed failure surface

- `PARSER_UNAVAILABLE`
- `PARSE_DEGRADED`
- `STRUCTURAL_FACT_UNMAPPED`
- `CONFIGURATION_AMBIGUOUS`
- `ENRICHMENT_BUDGET_EXHAUSTED`

## Exit tests / evidence

- `malformed_rust_fixture`
- `cfg_variant_separation`
- `definitions_references_tests_docs_fixture`
- `non_utf8_reject_or_map`
- `no_compiler_truth_overclaim`
- `no_execute_parser_boundary`

## Suggested internal modules

```text
search-code-enricher/src/
  profile.rs
  parser.rs
  facts.rs
  relations.rs
  roles.rs
  cfg.rs
  manifest.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Additional languages require independent profile modules and may become crates only with separate provider/dependency lifecycles.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

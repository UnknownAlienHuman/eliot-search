# Function contract — `search-code-enricher`

**Status:** W5/P10 logical contract; no Rust parser provider or artifact is selected.

The baseline profile extracts tolerant Rust syntax facts and relations. It does not type-check, execute
repository code, expand procedural macros, run build scripts or claim compiler truth.

## State and profile ownership

The package owns:

- one immutable qualified Rust parser/enrichment profile;
- bounded parser input/output and malformed-input policy;
- structural facts, relations, evidence roles and configuration predicates;
- provider assurance and explicit enrichment gaps;
- deterministic enrichment manifests and fixture corpus.

It owns no source bytes, source identity, query ranking, Qdrant transport, compiler/LSP process or
multi-language runtime.

## `validate_parser_profile`

```text
validate_parser_profile(candidate, qualification_receipt)
    -> Result<QualifiedRustParserProfile, EnrichError>
```

Requires exact parser/grammar package name, version, source checksum, license evidence, node/query schema
identity, Rust edition/dialect coverage, normalization rules, resource limits, malformed-input policy,
no-execute declaration and golden-fixture digest.

`latest`, semver ranges, floating Git revisions, undocumented defaults and a successful parse alone are
not qualification. Any behavior-affecting change yields a new profile digest and requires re-enrichment/
projection generation as classified by the integration plan.

## `parse_rust_no_execute`

```text
parse_rust_no_execute(representation, profile, budget, cancel)
    -> Result<RustSyntaxTree, EnrichError>
```

**Preconditions**

- exact immutable representation and coordinate map are available;
- input encoding/size is supported and bounded;
- profile digest matches the accepted qualification receipt;
- no external parser process, repository build action or network access is permitted unless separately
  admitted by a future profile.

**Postconditions**

- syntax nodes are bounded, deterministic and anchored to exact representation coordinates;
- parser diagnostics and recovery nodes are preserved as explicit degraded state;
- source text is not copied into ordinary telemetry/receipts;
- cancellation/deadline returns no manifest claiming complete parse.

Parser callbacks, injections or queries cannot execute repository code, build scripts, macros, shell
commands, credential prompts or remote resources.

## `extract_structural_facts`

```text
extract_structural_facts(tree, representation, units, profile, budget, cancel)
    -> Result<StructuralFactSet, EnrichError>
```

Extracts only profile-registered fact kinds such as module, function, method, type, trait, impl, field,
constant, static, macro invocation, test, documentation section and configuration item.

Every fact carries:

- deterministic fact identity;
- source revision/representation/unit identity;
- native anchor and fact digest;
- parser profile digest;
- assurance class (`tolerant_syntax` baseline);
- evidence role;
- configuration predicate reference;
- bounded attributes from a closed registry;
- degradation/gap reasons.

A syntax node does not become a definition/reference solely because its display text resembles one; the
profile owns explicit extraction rules.

## `extract_structural_relations`

```text
extract_structural_relations(tree, facts, profile, budget, cancel)
    -> Result<StructuralRelationSet, EnrichError>
```

Produces bounded descriptive relations such as contains, declares, implements_syntactically,
invokes_syntactically, references_name, tests_subject, documents_subject and guarded_by_cfg.

Relations are not compiler-resolved call graphs, trait resolution, type identity or proof of runtime
behavior. Unknown/ambiguous targets remain explicit and cannot be silently bound by name alone.

## `classify_evidence_role`

```text
classify_evidence_role(node, ancestry, attributes, profile) -> EvidenceRoleDecision
```

Returns a closed role and reasons: definition, reference, test, documentation, caller, configuration or
unknown. Tests/docs/callers are evidence categories, not automatic truth or ranking authority.

## `extract_configuration_predicate`

```text
extract_configuration_predicate(node, representation, profile)
    -> Result<ConfigurationPredicate, EnrichError>
```

Preserves a canonical bounded AST for `cfg`, `cfg_attr` and relevant feature/target predicates:
`all`, `any`, `not`, key and key-value. It does not evaluate a predicate without an explicit target/
feature environment. Unsupported macro-generated or malformed predicates return `unknown` plus exact
anchor/reason, never unconditional applicability.

Configuration variants remain separate in facts, relations, manifests and comparison inputs.

## `validate_fact_anchor`

```text
validate_fact_anchor(fact, representation, coordinate_map)
    -> Result<AnchorValidationReceipt, EnrichError>
```

Reopens/uses the exact representation, checks revision/profile/fact digests and maps the native anchor.
Lossy or ambiguous mapping lowers assurance or rejects the fact; raw-byte exactness is never fabricated.

## `build_enrichment_manifest`

```text
build_enrichment_manifest(request, facts, relations, gaps, limits)
    -> Result<EnrichmentManifest, EnrichError>
```

Produces canonical deterministic ordering, exact counts/digests, profile/representation/unit identities,
assurance summary, configuration-predicate registry and explicit incomplete/degraded gaps.

A manifest with parse recovery or budget omissions cannot claim compiler-complete structure. No vendor
syntax node type crosses the package boundary.

## `compare_profile_change`

```text
compare_profile_change(old, new) -> EnrichmentProfileChange
```

Returns `NOOP`, `RE_ENRICH_AND_REPROJECT`, `GATE_REQUIRED` or `REJECT`. Parser/grammar/query schema,
Rust dialect, extraction rules, assurance semantics, predicate parsing, coordinate behavior or bounds
changes require a new profile identity and re-enrichment. No live reinterpretation of old facts is
allowed.

## Cancellation, deadlines and crash semantics

Parsing/extraction/manifest creation are pure over immutable inputs. Equal inputs/profile yield equal
outputs. Cancellation or budget exhaustion returns no successful complete manifest; a caller may accept
a separately typed degraded manifest only when the recipe/assurance contract permits it and gaps are
preserved.

The package owns no durable mutation and no unknown external commit outcome. Rebuild after crash uses
immutable representation/unit inputs and the exact accepted profile digest.

## Typed failures

- `PARSER_PROFILE_INVALID`
- `PARSER_NOT_QUALIFIED`
- `PARSER_UNAVAILABLE`
- `RUST_INPUT_UNSUPPORTED`
- `RUST_INPUT_TOO_LARGE`
- `PARSE_DEGRADED`
- `PARSE_CANCELLED`
- `PARSE_BUDGET_EXHAUSTED`
- `STRUCTURAL_FACT_UNMAPPED`
- `STRUCTURAL_RELATION_AMBIGUOUS`
- `CONFIGURATION_AMBIGUOUS`
- `ANCHOR_MAPPING_FAILED`
- `ENRICHMENT_MANIFEST_INVALID`
- `ENRICHMENT_PROFILE_MISMATCH`

## Required tests / qualification evidence

- exact parser/grammar/source/license/profile identity and golden digest;
- deterministic parse/fact/relation/manifest bytes;
- malformed and incomplete Rust produces bounded degraded facts/gaps, never compiler truth;
- Rust editions, modules, functions, traits, impls, methods, fields, constants, macros and tests corpus;
- `cfg`/`cfg_attr` all/any/not/key/value and malformed/unknown variant separation;
- definitions, references, callers, tests, docs and configuration evidence-role fixtures;
- proc-macro/build-script/LSP/shell/network execution is structurally absent and runtime-probed;
- macro invocation is not silently expanded or compiler-resolved;
- non-UTF8/invalid coordinate behavior explicit;
- anchor/digest/profile mismatch rejection;
- cancellation and node/depth/byte/time limits;
- profile change requires re-enrichment/reprojection;
- public API contains no parser/vendor node type;
- default telemetry contains no source bytes, query text, secrets or unrestricted paths.

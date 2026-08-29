# `search-lexical` implementation packet

**Path:** `crates/search-lexical`  
**Capability:** C11  
**Delivery:** W3 / P06  
**Gate:** BLOCKED until Qdrant capability choice and W2 unit contracts are accepted  
**Trace:** S12, H9, P06  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Implement one deterministic lexical profile that emits compatible named sparse vectors for documents and queries.

## Owns

- LexicalProfileDescriptor and immutable analyzer identity
- tokenization/normalization/weighting for selected code and neutral-text profiles
- document/query sparse encoding
- golden compatibility and collision fixtures

## Must not own

- an inverted index or second search database
- automatic runtime switching between lexical providers
- implicit stemming, stopwords or language defaults
- using lexical collision-prone matches as exact proof

## Logical primitives

- LexicalProfileDescriptor, LexicalInput, TokenObservation, SparseVector, SparseFeature, LexicalEncodingReceipt, CollisionProfile

## Logical operations

1. `encode_document(input, profile) -> Result<SparseVector, LexicalError>`
2. `encode_query(input, profile) -> Result<SparseVector, LexicalError>`
3. `describe_profile() -> LexicalProfileDescriptor`
4. `validate_document_query_compatibility(fixture) -> Result<(), LexicalError>`
5. `measure_collision_corpus(corpus) -> CollisionReport`

## Required invariants

- one collection generation selects exactly one lexical provider path
- profile identity changes when tokenizer/hash/weighting/BM25 semantics change
- document and query fixtures match qualified reference semantics
- exact identifiers use exact branches, not BM25
- vector output is deterministic and bounded

## Typed failure surface

- `LEXICAL_PROFILE_MISMATCH`
- `LEXICAL_PROVIDER_NOT_QUALIFIED`
- `LEXICAL_COLLISION_RISK`
- `LEXICAL_INPUT_UNSUPPORTED`
- `LEXICAL_BUDGET_EXHAUSTED`

## Exit tests / evidence

- `snake_camel_pascal_qualified_golden`
- `unicode_identifier_and_path_golden`
- `no_implicit_stopwords_or_stemming`
- `document_query_reference_compatibility`
- `collision_corpus_measurement`
- `provider_switch_requires_generation_change`

## Suggested internal modules

```text
search-lexical/src/
  profile.rs
  tokenize.rs
  normalize.rs
  weight.rs
  sparse.rs
  collision.rs
  fixture.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Code and neutral-text profiles remain together only while sharing one tokenizer/runtime dependency. Split on a real provider or dependency boundary.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

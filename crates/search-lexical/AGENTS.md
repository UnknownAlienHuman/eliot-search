# Agent contract — search-lexical

You own only `crates/search-lexical/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S9.4, S12, H9, P06.

## Mission

Encode documents and queries into immutable sparse-vector profiles without owning an index.

## Ownership

- code_v1 and text_neutral_v1 profile behavior
- tokenization, Unicode and identifier expansion
- document/query compatibility fixtures
- term-index/collision policy
- profile and fixture digests

## Forbidden ownership

- inverted-index or searchable corpus storage
- implicit English stopwords/stemming
- runtime fallback between providers
- using BM25 to prove exact identity or absence

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `LexicalEncoder::profile() -> LexicalProfileDescriptor`
- `LexicalEncoder::encode_document(input) -> Result<SparseVector, LexicalError>`
- `LexicalEncoder::encode_query(input) -> Result<SparseVector, LexicalError>`
- `LexicalEncoder::fixture_digest() -> Digest`
- `validate_document_query_compatibility(fixture) -> Result<(), LexicalError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `LEXICAL_PROFILE_MISMATCH`, `LEXICAL_FIXTURE_MISMATCH`, `COLLISION_PROFILE_UNQUALIFIED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `snake/camel/Pascal/qualified identifiers match golden vectors`
- `Unicode and paths follow pinned normalization`
- `no implicit stopwords or stemming`
- `document/query vectors match qualified reference fixture`
- `provider/profile change requires new collection generation`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W3 / P06**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

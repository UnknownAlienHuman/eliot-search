# search-domain — remove the planned-system algebra

This package is not a useful business-domain core. It is about **80.9 KiB of production Rust** which
centralizes proof, fingerprint, currentness, coverage, eligibility, visibility, transition and mutation
vocabularies from the historical Architecture 8.4 programme.

Most exported operations have no external production caller. The remaining callers belong either to
mechanisms already being removed or to one small helper which can live beside its real owner.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and local [source/caller map](FUNCTIONS.md).

## Verified caller disposition

No external caller was found for the public assurance, currentness, plan-fingerprint, eligibility, stable
candidate-order, coverage or visibility evaluators. Searches for their concrete operation names resolve only
to package source/tests.

External uses found:

- `transition_publication` — old generic `PublicationIntent` persistence/visibility code being replaced by one
  `PendingPublication` + `CurrentPublishedView` transaction;
- `transition_source_ownership` and `source_ownership_v2` — old namespace cutover/registry/record schemas being
  deleted with the source registry stack;
- `classify_mutation_outcome` — runtime-owner/development code; a small generic enum/function around actual
  owner failure semantics;
- Cargo dependencies from many packages whose old APIs already import domain types but whose current target
  no longer needs this package.

A package dependency or re-export is not a consumer of useful behavior.

## Target

Domain rules live with the operation which owns the data/effect:

| Current domain area | Direct owner |
|---|---|
| publication transition | `search-publication` pending/current/cleanup records |
| source head/ownership | typed current source/root owner |
| selected scope | query's `ResolvedScope` over the pinned view |
| coverage/complete negative | current `SearchResponse` / explicit `Exact` operation |
| ranking/order | Qdrant/native fusion or the small real rank merge |
| evidence assurance | actual manifest/object verification and explicit gaps |
| currentness | immutable `PublishedViewGuard` and current source/view references |
| visibility/readiness | actual constructed handler/component state |
| mutation outcome | concrete effect owner; unknown only after a possible real effect |

Do not replace this package with `domain-v2`, another policy kernel or a directory of generic enums.

## Potentially useful small code

Retain a helper only when at least two current direct owners use exactly the same semantics after cutover.
Otherwise move it beside its caller.

The current `classify_mutation_outcome` may be reduced to direct effect-owner logic:

```text
before dispatch / known abort     → known failure
possible external/durable effect  → unresolved until authoritative readback
verified postcondition            → success
contradictory actual state        → blocked/corrupt
```

Publication, redb, object writer and process ownership have different readback/retry actions; a universal
retryability enum is not automatically useful.

## Removal boundary

Delete after direct callers move:

- assurance aggregate and reason/proof records;
- coverage classification over old exact reports;
- currentness axes, snapshot-drift/fingerprint comparisons and revalidation policy;
- plan/query fingerprint construction and verification;
- eligibility AST/set/equivalence/digest proofs;
- generic stable candidate ordering based on old assurance/fingerprint fields;
- source ownership v2/cutover generation and generic transition state machine;
- old publication transition over `PublicationIntentState`;
- visibility requirements over process/binding/grant/profile booleans;
- generic `Decision`, `ReasonSet` and `DomainError` surfaces after consumers disappear;
- package fixtures/property tests protecting those APIs;
- unused `search-domain` Cargo dependencies from consumer packages.

Keep concrete checked arithmetic and deterministic ordering in the actual owner which uses them. Do not leave
forwarding re-exports.

## Completion

The current source, publication, query, evidence, readiness and failure paths compile without `search-domain`.
Every retained rule is visible in its owning operation and tested through its real inputs/outputs. The package
is deleted unless a small, independently meaningful multi-consumer kernel remains after the cutovers.

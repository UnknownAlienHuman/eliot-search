# ELIOT Search — implementation

Read the [architecture](../architecture/README.md) for purpose and observable behavior. This guide selects the
current mechanisms, owners and delivery order. Nearest `FUNCTIONS.md` files own exact code seams; the
[analysis registry](../audit/ANALYSIS_REGISTRY.md) owns revision-specific evidence and status.

The original source baseline is `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`. The current branch is a
pre-release refactor: correct existing code may remain, internal formats need no compatibility facade, and a
package name or feature does not establish connected behavior.

## Target path

```text
selected source
→ stable acquisition and retained bytes
→ current source head
→ recoverable preparation and units
→ real Qdrant effects
→ atomic published metadata and view
→ scoped query or exact operation
→ pinned evidence lookup and retained excerpt
→ CLI/local-client result
```

Every arrow has one owner. Durable input/outcome comparison recovers lost wake-ups; stale work cannot overwrite
current state. Unsupported, empty, partial and failed outcomes remain distinct.

## Owners

| Responsibility | Single current recipe |
|---|---|
| Root/config | [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [config](../../crates/search-config/FUNCTIONS.md) |
| Source acquisition/current rows | [source overview](../../crates/search-source/README.md), [registry](../../crates/search-source/search-source-registry/FUNCTIONS.md), [reconcile](../../crates/search-source/search-source-reconcile/FUNCTIONS.md) |
| Immutable retained objects | [revision store](../../crates/search-source/search-revision-store/FUNCTIONS.md), coordinated with #598 |
| Preparation and unit continuity | [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unitizer](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Retrieval identity and metadata | [point identity](../../crates/search-index-qdrant/search-point-identity/FUNCTIONS.md), [planner](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md), [control](../../crates/search-control-redb/FUNCTIONS.md) |
| Qdrant effects/publication/cleanup | [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md), [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) |
| Query, exact, evidence and windows | [query](../../crates/search-query/README.md), [exact](../../crates/search-query/search-exact/FUNCTIONS.md), [window](../../crates/search-query/search-continuation/FUNCTIONS.md) |
| Protocol, child lifetime and CLI | [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [client](../../crates/search-provider-client/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
| Stopped-root collection/archive/import | [maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |

An owner is a responsibility, not a required crate. Move useful behavior and real callers together; remove the
replaced mechanism, exclusive fixtures and dependency edges in the same slice where practical.

## Delivery sequence

Independent preparation may proceed in parallel, but this is the shortest dependency order for a working
application:

| Slice | Observable result | Primary issues |
|---|---|---|
| 1. Invocation, settings and root | One parsed invocation; validated typed settings reach actual constructors; explicit inspect/init/open and factual status | #333, #266, #360 |
| 2. Source, object and preparation | One selected file is acquired once, retained, installed as current under an ABA-safe guard, prepared after restart and reconciled after lost wake-ups | #267, #270, #330, #331, #271 |
| 3. Identity, Qdrant and publication | Stable retrieval identities, one real SDK runtime, atomic published evidence/view, restartable rebuild and finite cleanup | #259, #329, #263, #261, #300, #303 |
| 4. Query, protocol and CLI | Explicit scope reaches Qdrant or Exact, resolves pinned retained evidence, supports bounded same-session paging and truthful cancellation/gaps | #272, #279, #291, #293, #275, #280, #332, #264 |
| 5. Maintenance | Stopped-root object collection plus archive and fresh-destination import | #302, #244 |
| 6. Product depth | Source-backed exact, orientation/comparison, documents and Git behavior through the same path | #249, #281, #227, #335; optional #219/#334/#336 |

A slice is complete only when the real producer reaches the real consumer, displaced paths and fixtures are
removed, canonical documentation and the registry are updated, and the smallest relevant compiler/Clippy or
runtime evidence is recorded. A constructor, receipt, feature flag or closed issue is not completion.

## Core implementation rules

### Source and preparation

Capture OS arguments/config once. Inventory explicitly selected roots with conservative retirement: an
incomplete scan never proves absence. Acquire stable bytes once, retain them before installing the current head,
and compare a non-reset current-row/registration guard so A → B → A and remove/re-add cannot revive stale work.

One materialization feeds unitization and one prepared artifact. Do not reparse between adjacent trusted stages
or maintain a second object writer. Supported complete empty input yields zero units; unsupported, truncated or
cancelled input is an explicit gap. Reuse transformed work only when the actual parser/profile/context inputs
match; source occurrence continuity follows the unitizer's one-old/one-new rule, not duplicate position.

### Evidence, publication and Qdrant

Qdrant is derived retrieval state, not source evidence. redb stores compact current/published rows and real read
snapshots; immutable objects hold bodies and larger unfinished effect pages. Published query-visible retrieval
state is immutable for an old view. Changed meaning creates a new point or generation; citation-only changes stay
in versioned evidence rows.

Publication fixes pending inputs before external effects, distinguishes observed state from unresolved late
writers, commits evidence/inventory/current view atomically and installs the matching snapshot before visibility.
Cleanup waits for relevant readers and possible writers. A timeout, reconnect or matching readback is not a fence
against an earlier dispatched mutation.

Use one `qdrant_client::Qdrant` runtime for current publication and query operations. Native BM25 provides
candidate order. The base population used for IDF is separate from request-only narrowing; an empty or missing
statistics population must not silently widen or become proof of no content.

### Query, exact and session lifetime

A request pins one published view, resolves a non-widening scope and owns one absolute deadline/cancellation plus
finite read/output limits. Select the page before source I/O, resolve exact metadata and verify retained bytes
before emitting excerpts. Ranked top-k is navigation, never exhaustive absence.

Exact scans the stated pinned inventory independently of ranked nominations. Complete no-match requires every
selected item in the declared representation to be verified and scanned without gaps.

One process-local query window belongs to one live session. Continue/Expand never rerun the corpus or expose a
locator destroyed on one-shot child exit. Cursor/window state advances only after complete bounded terminal
output; partial output fails the session and commits no progress.

### Maintenance and optional depth

Local final-object collection is explicit stopped-root maintenance. Archive/import creates a fresh destination
and never runs copied pending external operations against the original Qdrant collection. Protected storage
uses only the accepted #598 object/key seam; do not inherit its retired six-domain/purge programme.

Navigation, comparison, documents and Git remain product requirements. Add syntax, model, document, overlay or
MCP dependencies only with a real source-backed consumer using the same evidence path. Optional capability is
not a reason to preserve an unavailable worker or build a provider registry.

## Evidence policy

Use Rust 1.98 and locked dependencies. Run minimal compiler/Clippy for changed production owners and immediate
consumers after code. Run focused causal/native/recovery/scale cases only when the implementation exists.
Record exact SHA, platform, command and result in the registry.

Current unrun product cases include lost wake-up, stale completion, empty/gap transitions, same-root restart
versus restored copy, exact retained lookup, duplicate edits, old-reader growth, delayed Qdrant mutations,
immutable old-view retrieval, isolated-generation recovery, stopped-root collection, blocked I/O and actual
retrieval quality/scale. Historical audit evidence explains counterexamples; it does not certify closure.

# search-handles — remove the second result store

A source-backed result does not need a separately minted authority. The card carries its exact source, revision,
representation, location and verified excerpt. Optional expansion is a locator into the same live
`QueryWindow` and pinned view.

Read the [query cutover](../README.md), [query-window map](../search-continuation/FUNCTIONS.md) and local
[source map](FUNCTIONS.md).

## Revision-specific evidence

At the established audit revision, the shared package measured about 35 KiB and the daemon had another roughly
19 KiB handle catalogue. The audit found whole-source-catalogue scans/hashes, per-item random tokens, duplicated
source/revision records and arbitrary full-revision range expansion. It also found that the public daemon did not
use the shared package. These measurements and caller findings are historical evidence, not current branch truth.

Before deleting either model, search current manifests, Rust imports/constructors/path inclusion, non-Rust
tooling, features, targets, tests and workflows. Move every surviving protocol/result/daemon consumer in the
same cut. Do not migrate one old store into the other.

## Replacement

A normal `EvidenceCard` contains source/revision/representation, exact location, verified excerpt,
rank/contribution metadata and optionally:

```text
WindowItemRef { window_id, item_index }
```

The locator resolves only the exact successfully emitted item in one live `QueryWindow`. Expansion derives a
finite relative neighborhood from that item's pinned manifest entry, clamps it to configured and immutable-object
boundaries, reserves read/output budget and verifies retained bytes. It may not name arbitrary absolute offsets,
another item or another view.

Useful mechanics to keep are exact immutable-range reading, actual range/length verification, finite expansion
limits, an exclusive prepared-output guard and the original absolute expiry/deadline check. Source authority
comes from the pinned manifest and actual reader, not echoed digests or copied handle records.

For owning-account child stdio, `(window_id, item_index)` is process-local session state, not a cryptographic
bearer token. A selected restricted-client adapter binds and cancels its own windows directly.

## Removal boundary

After caller cutover, delete the shared handle store, daemon result-handle catalogue, entropy/token hashing,
whole-catalogue fences/scans, handle grants/permits/invalidation generations, arbitrary revision offsets,
durable retention leases and handle-specific crypto/provider/shape fixtures. Move useful read/output mechanics
into the query-window/evidence owner and leave no facade.

## Completion

Cards emit without per-item token minting or whole-catalogue scans. Expansion can enlarge only the selected
item's context under the same view/window and fails after expiry/restart/session revocation. After code, run
minimal Rust 1.98 check/Clippy for actual query/window/evidence/protocol/daemon consumers.

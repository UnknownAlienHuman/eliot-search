# Working on search-handles

Read the [query cutover](../README.md), [query-window map](../search-continuation/FUNCTIONS.md) and local
[source map](FUNCTIONS.md).

Do not repair or integrate either current handle store. Normal source-backed cards contain their verified
excerpt/location directly. Optional expansion uses `(window_id, item_index)` inside the same live
`QueryWindow` and pinned view.

Move only useful mechanics:

- exact immutable range read and actual byte/range verification;
- finite expansion/output limits;
- exclusive prepared expansion through output;
- original absolute expiry/deadline check.

Do not mint random per-item tokens, enumerate/hash the whole source catalogue, copy source/revision authority
into another record or allow arbitrary absolute offsets across the revision. Expansion derives a bounded
neighborhood from the exact emitted item's `ViewManifestEntry`.

Owning-account local use needs no cryptographic bearer handle. A real restricted-client adapter binds and
cancels the window through its concrete session. Remove handle grants, permits, invalidation generations,
durable retention leases and crypto/provider seams with consumers.

Change protocol/results/daemon callers together and delete this crate plus the daemon result-handle module.
After code, run minimal Rust 1.98 compiler/Clippy for actual query/window/evidence/protocol consumers.

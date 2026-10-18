# search-continuation — removal map

This package is a second in-memory continuation lifecycle. It stores token-digest records, candidate
windows or durable-replan descriptors, issued fingerprints, security generations and cleanup effects.
The public daemon does not use it for paging; it uses its own
[`ContinuationCatalog`](../../../bins/eliot-searchd/src/continuation/kernel/catalog.rs) plus a separate
result-handle catalogue.

The shared package contains well over 160 KiB of Rust source before its security submodule is counted.
Most of that code protects a durable/replayable query capability that the baseline local product does not
need.

## One query window

Keep one application-owned `QueryWindow`:

```text
window_id
PublishedView guard
final ordered EvidenceRef[]
next_index
optional retained excerpts / issued item locators
absolute expiry
finite item, byte and output limits
selected session binding only for a real restricted-client profile
```

Reuse the daemon's useful slice/cursor and
[`PreparedPage::deliver`](../../../bins/eliot-searchd/src/continuation/kernel/catalog/prepared.rs)
mechanics: prepare the next page, emit it, and advance only after the complete output write succeeds.

Do not recompute a whole source-catalog digest on every page. The pinned published view is the source and
index fence. Read and verify only selected-page evidence. Completion, expiry, disconnect, cancellation or
restart drops the process-local window and releases its view guard. A caller may repeat the query; there is
no durable replan or cleanup journal.

An optional `WindowItemRef` resolves an item in this same window. Finishing pagination does not discard
items still needed by issued locators before the original expiry.

## Cutover

Move the useful daemon paging mechanics into the serving query owner, change `ContinuationHandle` and
`SearchSourceHandle` result contracts together, migrate native lifecycle consumers, and delete both the
shared continuation package and duplicate daemon catalogues after their last callers.

[FUNCTIONS.md](FUNCTIONS.md) records current repeated work and the removal sequence.

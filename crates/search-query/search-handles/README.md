# search-handles — removal map

This package is an in-memory handle lifecycle that duplicates the daemon's
[`ResultHandleCatalog`](../../../bins/eliot-searchd/src/result_handles/kernel/catalog.rs). The public
query already has a continuation window and then mints another token/record for every emitted match.

The package's two Rust files contain about 35 KiB of source before tests and documentation. They add
token digests, bindings, grants, durable-handle claims, invalidation cursors and terminal records, but no
production persistence or retention lease. Expiry/invalidation marks records terminal without removing
them, so long-running use can grow history even when live quotas remain below their limit.

## Target

A normal result is an `EvidenceCard` containing source/revision/representation, exact location/range and
a short excerpt. Optional expansion uses a `WindowItemRef` into the same short-lived
[query window](../README.md#one-query-window). The window already owns the pinned published view,
ordered evidence references, expiry and byte limits.

Keep only mechanics that serve that path:

- exact range and retained-revision readback;
- actual byte/deadline/output limits;
- selected restricted-client scope checks when that client exists;
- deletion of expired/terminal state.

Do not preserve a separate handle identity, token-digest database, durable source-handle profile,
retention lease, replay history or per-result cryptographic authority for the owning local client.

## Cutover

Change `SearchSourceHandle` fields in `search-contracts`, recipes, evidence frames, ports, provider
codecs, projector and daemon callers together. Results become evidence cards plus an optional window-item
locator. Move any useful readback code to the retained-source reader/query window, switch all consumers,
then delete this package and the daemon `result_handles` catalogue.

[FUNCTIONS.md](FUNCTIONS.md) records the current API and exact removal boundary.

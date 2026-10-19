# search-continuation — replace every continuation store with one QueryWindow

The baseline product needs short-lived paging over one already ranked result set. It does not need a durable
query capability, replay journal, issued-fingerprint history, security-generation graph or replan protocol.

Read the [query cutover](../README.md), [daemon map](../../../bins/eliot-searchd/FUNCTIONS.md) and
[source map](FUNCTIONS.md).

## Verified current source

This shared package is not the public daemon's paging owner. Its production files already exceed 79 KiB
before the nested security directory is counted; tests add much more. It stores token-digest records,
candidate windows or durable-replan descriptors, issued fingerprints, security generations and external
cleanup effects.

The public daemon instead has its own `ContinuationCatalog`. It retains a complete `Vec<StoredMatch>`, cursor,
expiry, namespace/session tag and a SHA-256 fence over every current source. Every continuation page
re-enumerates and rehashes the whole source catalogue. The same page then enters a separate result-handle
catalogue which enumerates the catalogue and computes another full fence.

Do not migrate the daemon to the shared lifecycle package. Replace both.

## One application window

```text
QueryWindow {
  id
  PublishedViewGuard
  final ordered EvidenceRef[]
  next_index
  optional issued-item visibility / cached verified excerpts
  absolute expiry
  finite item / metadata-byte / source-read / output limits
  optional concrete restricted-session binding
}
```

`EvidenceRef` is enough information to resolve a stable ranked point through the pinned view manifest. It is
not a second source snapshot, backend cursor or public authority record.

The window is process-local. Restart, daemon shutdown, session disconnect, explicit cancellation or expiry
may invalidate it. A caller may repeat the query. No durable continuation recovery is required for ordinary
interactive search.

## Page delivery

Preserve the useful mechanic from the daemon's `PreparedPage`:

1. hold an exclusive mutable borrow of the selected window;
2. choose `[next_index .. next_index + page_size]` without rescanning earlier results;
3. reserve request/window read and output budget;
4. resolve and verify only that page's retained evidence under the original absolute deadline;
5. format one bounded response in the established final order;
6. write the complete frame;
7. advance `next_index` only after complete output success.

A failed or partial frame advances zero. The connection/session fails closed and may drop the window so a
partial response is never treated as a recoverable acknowledgement.

For the first page, stage the new window and insert it only after successful output. Do not expose a token
for a window which was never committed.

## Item expansion

Cards normally contain the source, exact revision/representation/location and a short verified excerpt.
When more context is useful, return:

```text
WindowItemRef { window_id, item_index }
```

Expansion resolves that exact item under the same view guard and expiry. It may return a bounded configured
neighborhood around the manifest range. It cannot select another item, another view or arbitrary bytes from
the entire revision. The immutable reader verifies the object/range again or reuses a verified cached slice.

The final page may mark the cursor exhausted while keeping the window until expiry if issued item locators
remain valid. Without locators/caching, the exhausted window can be dropped immediately.

## Limits and identity

The window manager enforces finite:

- live-window count;
- total retained evidence-reference count and metadata bytes;
- per-window result count and TTL;
- per-page item/source-read/output ceilings;
- optional cached excerpt bytes.

For the owning-account local pipe, the ID is a process-local locator, not a cryptographic bearer grant. A
selected restricted-client adapter binds windows to its actual connection/session and cancels them on
revocation. Do not add token signatures, grant digests, purpose leases or durable secret storage to the
baseline.

## Remove after cutover

Delete:

```text
crates/search-query/search-continuation/
bins/eliot-searchd/src/continuation/ legacy catalogue
shared durable-replan / issued-set / security / cleanup lifecycle types
source-catalog fence hashing for pages
continuation token crypto/provider seams
continuation-specific contract compatibility families
```

Move only the direct slice/cursor and commit-after-output behavior to the real query owner. Change current
protocol/result callers together; do not retain forwarding wrappers.

## Completion

Page N preserves final rank, reads only its selected evidence, does no whole-corpus/source-catalog scan,
advances only after complete output and retains the old view exactly while the window is live. Expansion
cannot escape the selected item/view. Restart invalidates windows and releases guards. No second continuation
store remains reachable.

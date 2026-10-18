# Query and results

The [implementation guide](../../docs/implementation/README.md) connects one typed request to one
published view, retrieval, retained-source validation and compact evidence. Existing packages are donor
code and current locations, not permanent product boundaries.

## Target flow

```text
request + scope + limits
→ pin current PublishedView
→ Qdrant / exact / selected overlay retrieval
→ preserve final rank
→ resolve ViewManifestEntry
→ select one bounded page
→ read and verify its retained ranges
→ emit EvidenceCard[]
→ optionally retain one QueryWindow for another page or bounded expansion
```

`EvidenceCard` carries the source/revision/representation, location/range, short excerpt, rank/freshness
and useful gaps. It does not require a separately minted source capability. `WindowItemRef` is only an
opaque `(window, item)` locator for a result already selected under the window's pinned view.

## One query window

The current public daemon uses two stores for one response:

- [`ContinuationCatalog`](../../bins/eliot-searchd/src/continuation/kernel/catalog.rs) keeps the ordered
  `StoredMatch` vector, cursor and expiry;
- [`ResultHandleCatalog`](../../bins/eliot-searchd/src/result_handles/kernel/catalog.rs) creates another
  record and random token for every emitted match.

The next-page path then recomputes a whole-catalog source fence and rebuilds the source map before reading
the page. This work is independent of page size and duplicates facts already fixed by a published view.

Replace both with one application-owned `QueryWindow`:

```text
QueryWindow
  window_id
  PublishedView guard
  final ordered EvidenceRef[]
  next_index
  optional retained excerpts for issued cards
  absolute expiry
  selected client/session binding only when that profile exists
  finite item/byte/output limits
```

Keep the daemon's useful `PreparedPage::deliver` rule: select the next slice, prepare the response and
advance the cursor only after the complete output write succeeds. A partial frame closes the session and
drops the uncommitted preparation; it does not need a remote acknowledgement protocol or durable journal.

Read and validate only the page being emitted. Preserve upstream fused order. A final page may remove the
cursor, but the window remains until expiry while an issued `WindowItemRef` still needs its view/excerpt.
Completion, expiry, disconnect or cancellation removes the record and releases its view guard. Restart
invalidates all windows; the caller repeats the query.

For the owning local OS account, the window identifier is a locator, not authorization. A selected
restricted-client adapter may bind it to its real connection/session and disclosure policy. Do not make
that optional profile a mandatory cryptographic handle system for local CLI paging.

## Current owners and cutover

| Existing code | Keep | Remove or change |
|---|---|---|
| [access](search-access/) | Real selected scope and any actual restricted-client rule. | Mandatory grants/proof graphs for the owning local client. |
| [query planner](search-query-planner/) | Query/profile/limit selection. | Externally manufactured task plans and fingerprints. |
| [retrieval executor](search-retrieval-executor/) | Real backend calls and contribution merge. | Raw-score reordering across populations and unconditional exact counts. |
| [candidate validator](search-candidate-validator/) | Manifest lookup and actual retained-byte/range verification. | Digest-field equality presented as byte validation. |
| [result projector](search-result-projector/) | Shape selected verified evidence into bounded cards/gaps. | Handle-oriented metadata, lifecycle ownership and re-sorting final rank. |
| [continuation](search-continuation/) | Offset/page preparation ideas only. | Shared lifecycle/security/durable-replan platform after callers move. |
| [handles](search-handles/) | Effective range/read limits and useful readback checks only. | Independent handle store, durable handle profile and terminal history. |
| [exact](search-exact/) | Explicit bounded source scan with truthful coverage. | Corpus-sized preload and duplicate per-offset matching. |

Change `SearchSourceHandle` / `ContinuationHandle` result fields in `search-contracts`, recipes, evidence
frames, ports, provider codecs and actual callers in the same cutover. Do not leave forwarding wrappers
that keep the old stores alive. The replacement contract is `EvidenceCard` plus optional next-window and
item locator; exact names can follow the existing V2 evidence model, but there is one current family.

## Verification

Verify the integrated caller, not package shape:

- page 1 and page N preserve the executor's final order;
- only selected-page source ranges are read;
- an unrelated source update does not rescan or invalidate a pinned old view;
- old evidence remains readable while its view guard is alive;
- complete output advances once; partial output advances zero times;
- expansion cannot leave the selected item/view and obeys byte/deadline limits;
- terminal/expired windows disappear and capacity is reusable;
- restart invalidates ephemeral windows without recovery records;
- no second handle/continuation database remains in the product path.

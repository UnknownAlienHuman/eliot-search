# Query and results

The [implementation guide](../../docs/implementation/README.md) connects one typed request to one
published view, retrieval, retained-source validation and compact evidence. Existing packages are donor
code and current locations, not permanent product boundaries.

## Target flow

```text
request + scope + limits
→ pin current PublishedView
→ select real constructed operations
→ Qdrant / exact / selected overlay retrieval
→ preserve final rank
→ resolve ViewManifestEntry
→ select one bounded page
→ read and verify its retained ranges
→ emit EvidenceCard[]
→ optionally retain one QueryWindow for another page or bounded expansion
```

`EvidenceCard` carries source/revision/representation, location/range, short excerpt, rank/freshness and
useful gaps. It does not require a separately minted source capability. `WindowItemRef` is only an opaque
`(window, item)` locator for evidence already selected under the window's pinned view.

## Direct operations, not a recipe operating system

The current contracts freeze eleven `RecipeIdV1` values, a caller-created `SearchTaskPlan`, capability
booleans, dependency/fingerprint records and a capability descriptor. Daemon query composition adds a
handler registry, projected descriptor, authority/grant host and service wrapper. Public-main search found
no implementation of `RegisteredAdmittedRecipeHandler` outside the registry definition, and
`compile_canonical_plan` has fixture callers only.

Replace that scaffolding with one current typed request family and direct dispatch to constructed handlers:

```text
Search    scope + query + retrieval options + limits
Exact     scope + literal/regex predicate + completeness limits
Continue  query_window + requested page size
Expand    window_item + bounded neighboring range/metadata request
Orient    optional source-guide dimensions/focus
Compare   optional selected subjects/axes
```

`Search` is the first connected mode. `Exact` is one server-owned operation, not public compile-plan then
execute-plan recipes. `Continue` and `Expand` use the same `QueryWindow`. `Orient` and `Compare` exist only
when their concrete prepared-fact/source handlers are implemented. Corpus profile/delta and provenance do
not need permanent recipe IDs before a product caller and data owner exist.

For the local CLI, a command calls the handler directly; an unsupported operation returns a typed
unsupported error. No capability handshake is needed. A selected restricted-client adapter may advertise
request versions, limits and the actual handler set, but advertisement is derived information and does not
replace session authorization or scope enforcement.

The planner may remain as a small pure helper for real multi-branch search. It accepts private references
to constructed services, the resolved scope, pinned view, query and limits. It does not accept a public
`CapabilitySet`, grant transcript, `SearchTaskPlan`, dependency graph or plan fingerprint. If lexical
search is one direct call, do not preserve a planner crate merely to manufacture a one-leg plan.

## One query window

The current public daemon uses two stores for one response:

- [`ContinuationCatalog`](../../bins/eliot-searchd/src/continuation/kernel/catalog.rs) keeps the ordered
  `StoredMatch` vector, cursor and expiry;
- [`ResultHandleCatalog`](../../bins/eliot-searchd/src/result_handles/kernel/catalog.rs) creates another
  record and random token for every emitted match.

The next-page path recomputes a whole-catalog source fence and rebuilds the source map before reading the
page. This work is independent of page size and duplicates facts fixed by a published view.

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
advance the cursor only after complete output succeeds. A partial frame closes the session and drops the
uncommitted preparation; it needs no remote acknowledgement protocol or durable journal.

Read and validate only the emitted page. Preserve upstream fused order. A final page may remove the cursor,
but the window remains until expiry while an issued `WindowItemRef` still needs its view/excerpt.
Completion, expiry, disconnect or cancellation removes the record and releases its view guard. Restart
invalidates all windows; the caller repeats the query.

For the owning local OS account, the window identifier is a locator, not authorization. A selected
restricted-client adapter may bind it to its real connection/session and disclosure policy. Do not make
that optional profile a mandatory cryptographic handle system for local CLI paging.

## Current owners and cutover

| Existing code | Keep | Remove or change |
|---|---|---|
| [query planner](search-query-planner/) | Useful selection/order/budget logic for real branches. | `CapabilitySet`, `SearchTaskPlan`, dependency/fingerprint proof and frozen recipe support tables. |
| daemon query composition | Real scope/published-view acquisition and eventual backend calls. | `CanonicalRecipeRegistry`, projected capability/authority ceremony and host layers without handlers. |
| provider protocol | Bounded framing, correlation, cancellation and one terminal result. | V1 recipe registry, paired local grants/MACs and duplicate V2 endpoint. |
| [retrieval executor](search-retrieval-executor/) | Real backend calls and contribution merge. | Raw-score reordering across populations and unconditional exact counts. |
| [candidate validator](search-candidate-validator/) | Manifest lookup and actual retained-byte/range verification. | Digest-field equality presented as byte validation. |
| [result projector](search-result-projector/) | Shape selected verified evidence into bounded cards/gaps. | Handle-oriented metadata, lifecycle ownership and re-sorting final rank. |
| [continuation](search-continuation/) | Offset/page preparation ideas only. | Shared lifecycle/security/durable-replan platform after callers move. |
| [handles](search-handles/) | Effective range/read limits and useful readback checks only. | Independent handle store, durable handle profile and terminal history. |
| [exact](search-exact/) | Explicit bounded source scan with truthful coverage. | Public compile/execute plan split, corpus preload and duplicate per-offset matching. |

Change recipe/request/result, planner, provider codec, dispatch and actual callers in one cutover. Do not
leave forwarding wrappers that keep old stores/registries alive or introduce a V3 family.

## Verification

Verify the integrated caller, not package shape:

- local `search` dispatch reaches the connected BM25 handler without capability credentials;
- unsupported optional operations are absent or return typed unsupported, not a weaker fallback;
- page 1 and page N preserve final order and read only selected-page ranges;
- old evidence remains readable while its view guard is alive;
- complete output advances once; partial output advances zero times;
- expansion cannot leave the selected item/view and obeys byte/deadline limits;
- terminal/expired windows disappear and capacity is reusable;
- restart invalidates ephemeral windows without recovery records;
- no second handle/continuation database, recipe registry or capability authority remains in the product path.

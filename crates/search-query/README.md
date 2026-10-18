# Query and results

The [implementation guide](../../docs/implementation/README.md) connects one typed request to one
published view, retrieval, retained-source validation and compact evidence. Existing packages are donor
code and current locations, not permanent product boundaries.

## Target flow

```text
request + selected source scope + limits
→ pin current PublishedView
→ resolve scope as a subset of that view / restricted session
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

## Scope comes from the published view

The local request selects configured sources or an explicit subset of the pinned `PublishedView`.
Unknown IDs fail and an explicit empty set never widens. One compatible selection feeds Qdrant retrieval,
`params.idf.corpus`, exact inventory and evidence validation. Split only incompatible routes/profiles.

A source update publishes a new view; an already pinned local request may finish coherently on its old
view. A selected restricted-client profile intersects the request with its authenticated session scope;
revocation closes/cancels that session and its windows. No grants, overlap proofs, scope digests, purge
generations or phase checkpoints are required for the owning-account path.

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
when their concrete prepared-fact/source handlers are implemented.

For the local CLI, a command calls the handler directly; unsupported operations return a typed error. No
capability handshake is needed. A selected restricted-client adapter may advertise request versions,
limits and actual handlers, but advertisement is derived information and not authorization.

The planner may remain a small pure helper for real multi-branch search. It accepts private references to
constructed services, the resolved scope, pinned view, query and limits. It does not accept public
capability/grant/task-plan/fingerprint records. If lexical search is one direct call, do not preserve a
planner crate merely to manufacture a one-leg plan.

## One query window

The current public daemon uses two stores for one response:

- `ContinuationCatalog` keeps ordered `StoredMatch` values, cursor and expiry;
- `ResultHandleCatalog` creates another record and random token for every emitted match.

The next-page path recomputes a whole-catalog source fence and rebuilds the source map before reading the
page. Replace both with one application-owned `QueryWindow` containing a `PublishedView` guard, final
ordered evidence refs, direct cursor, optional retained excerpts/item locators, absolute expiry and finite
item/byte/output limits.

Prepare the next slice, emit it and advance only after complete output. Read/validate only the emitted
page. Completion, expiry, disconnect or cancellation removes the record and releases its guard. Restart
invalidates windows; the caller repeats the query. The owning-account window ID is a locator, not bearer
authority; a restricted session may bind it to the real connection.

## Current owners and cutover

| Existing code | Keep | Remove or change |
|---|---|---|
| [access](search-access/) | Non-widening selected-scope resolution and a real optional restricted-session policy. | Local grants, overlap proofs, predicate digests, per-phase checkpoints and durable security barrier. |
| [query planner](search-query-planner/) | Useful selection/order/budget logic for real branches. | `CapabilitySet`, `SearchTaskPlan`, dependency/fingerprint proof and frozen recipe support tables. |
| daemon query composition | Real scope/view acquisition and backend calls. | Recipe registry, projected capability/authority ceremony and host layers without handlers. |
| provider protocol | Bounded framing, correlation, cancellation and one terminal result. | V1 recipes, paired local grants/MACs and duplicate V2 endpoint. |
| retrieval executor | Real backend calls and contribution merge. | Raw-score reordering across populations and unconditional exact counts. |
| candidate validator | Manifest lookup and actual retained-byte/range verification. | Digest-field equality presented as byte validation. |
| result projector | Format selected verified evidence into bounded cards/gaps. | Handle lifecycle and re-sorting final rank. |
| continuation / handles | Direct slice/cursor and actual read limits only. | Separate stores, durable replan, token authority and terminal history. |
| exact | Aho matcher, bounded source scan and truthful coverage. | Public plan/proof/checkpoint platform, corpus preload and duplicate matcher. |

Change request/result, scope, planner, protocol, dispatch and actual callers coherently. Do not leave
forwarding wrappers or introduce a V3 family.

## Verification

- local search reaches BM25 without capability/access credentials;
- selected sources never widen and excluded sources affect neither candidates nor IDF;
- exact and indexed paths use the same pinned population;
- unsupported optional operations fail truthfully;
- page N preserves final order and reads only selected ranges;
- old evidence remains readable while its view guard lives;
- partial output advances zero;
- restricted-session revocation cancels its windows;
- no duplicate handle/continuation, recipe/capability or access-checkpoint database remains.

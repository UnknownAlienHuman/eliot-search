# eliot-searchd — composition and command map

Follow the [implementation guide](../../docs/implementation/README.md). The daemon composes real effects
and direct handlers; library owners retain data and algorithm responsibilities. This map describes public
source at `c8e38b6` and the replacement path. Existing command, package, receipt and recipe boundaries are
not compatibility requirements.

## Current commands and missing connections

| Existing path | What the source actually does | Next implementation |
|---|---|---|
| [entry.rs](src/entry.rs) | Chains `maybe_refuse_legacy_effects` and several handlers which recapture arguments. | Parse one invocation/config value and dispatch one concrete operation. |
| native root / direct registry | Binds `registry_control` through `bind_original_v4`, restores the complete in-memory registry and original inputs, then republishes a full control snapshot for ordinary root/source work. | Point-read/update typed current root/source rows. Remove operation replay and snapshot publication from the normal path. |
| `--admit-source-v2` | Restores the registry, reads/hashes/classifies the file, commits admission metadata, drops the body and returns `retained_revision:false`. | Pass the same bytes into the single object writer, compare/set one source head and enqueue one preparation. |
| `--sync-source-roots` | Successful preflight ends with `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`. | Execute bounded inventory/update through the same ingestion owner. |
| legacy preparation commands | Prepare old retained objects through another storage engine. | Reuse useful materialization/coordinates with the one current object reader/writer. |
| public DIRECT service | Serves legacy read-only store commands; mutation is unavailable. | Keep exact source scan explicit and connect the one current typed protocol. |
| indexed query helper | `execute_pinned_leg` is typed directly to `ProcessTestIndexedPort`. Public search finds only its process test caller. It cannot accept the real Qdrant plane without replacing the API. | Directly call the one real Qdrant runtime from the current `Search` handler. |
| query authority/recipe composition | `registry.rs`, `service.rs`, `serving.rs` and `live_authority.rs` total about 51 KiB. `live_authority` is dead-code gated; no production constructors of `LiveNativeQueryAuthorityOwner`, `CanonicalRegisteredQueryService` or `CanonicalRecipeRegistry` were found, and no real `RegisteredAdmittedRecipeHandler` implementation was found. | Delete the host/registry/grant/capability ceremony and dispatch current typed operations to constructed handlers. |
| planner/executor/access/validator/projector | Roughly 104 KiB + 61 KiB + 52 KiB + 24.5 KiB + 10.3 KiB of production Rust. Their principal APIs are fixture/process-test callers, accept caller-shaped authority and do not connect retained evidence. | Keep only direct scope selection, bounded backend calls, manifest lookup, retained reads and card formatting; inline small remnants. |
| result paging | One daemon continuation catalogue retains `StoredMatch`; another catalogue mints one random source handle per emitted item. Shared handle/continuation packages form two more disconnected lifecycle systems. | Use one `QueryWindow` with a pinned view, final evidence refs, cursor and bounded item locator. |

Do not expose test-only methods by removing `cfg(test)` and call that integration. Do not wire the real
backend through old recipe, plan, ticket, grant, checkpoint or handle facades.

## Composition responsibilities

| Owner | Concrete responsibility |
|---|---|
| Invocation/root | One parsed command, one root lock, actual settings and orderly shutdown. |
| Source / object store | Stable acquisition and one immutable writer/reader. Bytes precede source-head commit. |
| Current source state | Typed redb root/source/current revision rows and bounded list/point reads. No operation archive or full restore before each command. |
| Preparation | One materialization and structure-first units. Unsupported transformations become gaps. |
| redb control | Current settings/source/head/artifact/publication rows in short transactions. No source bodies, completed-operation history or query history. |
| Publication | One pending artifact/cursor, Qdrant effect/readback and one committed `PublishedView`. |
| Qdrant | One private client/runtime used by publication and query. No qualification credential or process-local mutation ledger. |
| Query | Direct `Search`/`Exact` handlers over one pinned view, one resolved scope and one request context. |
| Evidence | Stable point → pinned `ViewManifestEntry` → exact retained object/range verification. |
| Query window | One process-local cursor/view guard for paging and bounded expansion. Restart may expire it. |
| Local protocol | One bounded typed request/result exchange. Actual constructed handlers determine availability. |

## Current source-state cutover

The normal direct command currently performs:

```text
restore full SourceRegistry
→ replay historical operation/original input layouts
→ mutate one root/source
→ read generic batch targets back
→ rebuild/publish full ControlSnapshot
```

Replace it with:

```text
open root owner once
→ point-read current root/source rows
→ perform stable source/object work outside redb writer
→ one compare/set typed transaction
→ update narrow current cache/pointer
```

Use bounded table iteration only for an actual inventory/publication snapshot. Move useful codecs to their
direct owner and delete `RegistryJournal`, original-input replay, generic `ControlJournalPort` and complete
registry reconstruction after callers move.

## Direct query dispatch

The local handler owns the actual resources:

```text
SearchRequest
→ pin current PublishedView
→ resolve non-widening ResolvedScope
→ RequestContext(absolute deadline, cancellation, finite counters)
→ real Qdrant BM25/native query
→ preserve final point order
→ ViewManifestEntry lookup
→ select one bounded page
→ retained object/range read and verification
→ EvidenceCard[]
→ optional QueryWindow
```

Dispatch current operations directly:

```text
Search    ranked source-backed evidence
Exact     one bounded complete source scan
Continue  next page from QueryWindow
Expand    bounded selected-item neighborhood from QueryWindow
Orient    optional only after a real source-backed handler exists
Compare   optional only after a real source-backed handler exists
```

The owning-account CLI needs no capability registry, descriptor digest, grant transcript, phase checkpoint
or plan fingerprint. A selected restricted-client profile may advertise actual handlers and must enforce
its real connection/session scope separately.

For one lexical branch there is no plan: call Qdrant. If multiple compatible Qdrant branches become real,
use native prefetch/RRF/grouping. A private local operation list exists only for genuinely incompatible
routes or a real non-Qdrant branch. It never becomes a public `SearchTaskPlan` authority.

## One request context

The request owner carries:

```text
PublishedViewGuard
ResolvedScope
absolute monotonic deadline
cancellation token
remaining backend nominations
remaining evidence items
remaining source-read bytes
remaining retained/output bytes
```

All Qdrant, object, formatting and output work consumes this context. No nested operation resets the
deadline. A process semaphore may bound concurrent requests through RAII; do not expose scheduler snapshots,
leg tickets or durable terminal maps.

Reads have no recovery journal. Cancellation releases content, window and view guards. The only durable
external uncertainty in the application is publication mutation, owned by `PendingPublication`.

## Evidence and output

A Qdrant hit is a nomination, not evidence. The query owner resolves the stable point in the pinned view's
manifest, verifies the exact retained object and range and then formats the card. Echoed payload/vector
hashes, caller readback digests and Boolean assurance records do not establish the bytes.

Select the output page before source I/O. Preserve final rank through lookup and projection. Group reads from
the same retained object where useful. A missing retained object/range is a candidate gap; a foreign,
conflicting or absent manifest point contaminates the backend result. Ordinary top-k remains
non-exhaustive.

The projector receives already ordered `VerifiedEvidenceItem` values. It may enforce encoded-output limits
and format cards/gap summaries. It must not re-sort raw scores, mint handles or receive prevalidated public
metadata. If that transformation stays trivial, inline it and delete the projector package.

## Query-window cutover

Current paging repeats whole-catalog work:

```text
continuation source_fence(store.list_sources)
+ handle source map(store.list_sources)
+ handle source_fence(store.list_sources)
```

Expansion repeats the fence and source scan again. Replace all stores with one window containing the pinned
view guard, final evidence refs, direct cursor, expiry and finite byte/item limits. The pinned immutable view
is the fence; unrelated source updates do not invalidate or rescan it.

Keep the daemon's useful exclusive `PreparedPage` rule: prepare the slice, resolve/emit it, then advance only
after complete output success. A partial frame advances zero and closes the session. Optional
`WindowItemRef(window_id, item_index)` can expand only that item's bounded neighboring range under the same
view and expiry; it cannot request arbitrary bytes from the entire revision.

## Remove replaced paths

- registry operation envelopes, original-input restore/requalification and full snapshot publication;
- migration/cutover/rollback frameworks for unsupported old roots;
- unused DEK/secret lifecycle outside a selected storage profile;
- legacy loopback/token-file transports and duplicate applications;
- V1 recipe registry, V2 parallel endpoint, authority bundle, capability projection and canonical query
  host;
- public plan/capability/grant/dependency/fingerprint records;
- scheduler/ticket/execution-state/terminal-map framework and process-test indexed backend;
- caller nomination/readback proof and raw-score projector sorting;
- duplicate handle/continuation stores, random per-item handles, source-fence rescans and lifecycle records;
- daemon Publisher/file journal after one publication owner is connected;
- source/module/shape tests and wave feature gates after their protected APIs disappear.

Unsupported old roots fail before writes and remain untouched. Do not replace deleted code with adapters,
synthetic grants, fake receipts, approved booleans or another registry.

## Verification

Verify the connected application: retain a real file across restart, prepare/index it, query from CLI and
reopen the correct excerpt. Then check one edit/rename, excluded scope and IDF population, foreign/missing
point, corrupt/missing evidence, old pinned view, page order, bounded expansion, exact scan, lost index,
cancellation and pending external publication.

Run minimal Rust 1.98 compiler/Clippy for changed production owners and actual consumers after code. Native,
live-Qdrant, recovery and scale claims require actual execution; constructors, fixtures, capability
records and historical qualification receipts do not establish them.

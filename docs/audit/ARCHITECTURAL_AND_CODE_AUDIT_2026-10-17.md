# ELIOT Search — distilled architecture and code audit

**Public source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`  
**Review period:** 2026-10-17–21  
**Status:** static refactor preparation; product/runtime qualification is false.

Read the [coverage and documentation map](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) before using
this report. It records which areas were source-traced, which remain partial and which runtime claims were
not executed. This report is intentionally shorter than the original audit; Git history retains the full
intermediate analysis.

Current direction is defined by the [architecture manifesto](../architecture/README.md) and
[implementation guide](../implementation/README.md). This audit explains why those cutovers are required. It
does not freeze package boundaries, pre-release bytes or the mechanisms found in public main.

## 1. Executive conclusion

ELIOT Search is not blocked by a lack of code. It is blocked by too many partially overlapping systems which
were developed before one basic application path worked:

```text
selected source
→ immutable retained bytes
→ one structural preparation
→ real Qdrant publication/query
→ exact retained excerpt
```

Public main contains large source, control, identity, transport, query, migration, security and
qualification platforms. Many are internally sophisticated, but their real producers and consumers are not
connected. Others duplicate an already existing owner or preserve a speculative future requirement.

The correct repair is not to complete every protocol. It is:

```text
select one current-format owner
→ connect the real producer and real consumer
→ use a mature library for standard work
→ remove the displaced mechanism and its tests/docs/tooling
→ verify the actual application behavior
```

No old internal API, S11 v1 key shape, receipt byte format, wave record, generated package map or
qualification token is a compatibility requirement. Unsupported pre-release roots may fail before writes
and remain untouched.

## 2. Measured baseline

The measurements below describe the inspected public commit. They are gross inventories, not promised net
deletion and not a substitute for caller analysis.

| Area | Measured inventory / finding |
|---|---:|
| Cargo workspace | 52 members |
| Tracked Rust | 1,593 files / 393,383 physical lines |
| Daemon Rust | 604 files / 122,212 physical lines |
| Test-named Rust outside xtask | 359 files / 101,276 physical lines |
| xtask Rust | 123 files / 23,823 physical lines |
| Canonical digest ledger | 3,572,618 bytes / 72,002 lines / 5,960 sites |
| `swarm/coverage` | 205 files / 152,921 lines |
| Source policy/receipt/record/history/identity/registry family | more than 1.22 MiB production source before safe-reader/object/reconcile/tests |
| Root/daemon owner composition | about 553.6 KiB before tests |
| Query proof/host stack | more than 300 KiB before protocol/paging |
| Provider/client/daemon protocol stacks | more than 500 KiB; shared contracts add about 486 KiB |
| Secret/key runtime | more than 310 KiB before daemon legacy migration/protection tests |
| Generic retention package | about 82 KiB plus restore/lifecycle composition |

The product itself is not proven to be 393k lines: the count includes comments, blanks, tests, harnesses and
unused code. The useful conclusion is that major families can be deleted or merged after real caller
cutover; chasing a target line count without ownership analysis would produce another rewrite.

## 3. Product topology and entrypoints

### Finding

Cargo targets, daemon dispatch, CLI paths and feature gates expose several applications:

- legacy DIRECT commands and stores;
- new root/source admission composition;
- process-test indexed query through an in-memory oracle;
- real Qdrant SDK code with no normal daemon query/publication caller;
- paired provider protocol and typed client;
- a separate public line/tab/hex runtime service;
- multiple test/harness binaries which resemble product entrypoints.

A compiled constructor, feature or capability descriptor does not make a product path.

### Decision

Keep one application:

```text
eliot-search CLI
→ one bounded child-stdio/local client
→ eliot-searchd direct dispatcher
→ current source/control/preparation/publication/query owners
```

Optional persistent pipe, restricted client, model, overlay, document parser or MCP leaf may be added only
after a real consumer is selected. Remove shadow entrypoints and feature topology with their last callers.

## 4. Root, configuration and current control state

### Findings

- Root ownership is surrounded by owner epochs, alternating slots, succession, migration borrowers, sealed
  co-holds and receipt/status records, while the local daemon needs one actual process-lifetime exclusion
  guard.
- Read/open/status paths can trigger initialization or migration-like effects.
- Configuration is parsed into a custom tree/registry/diff/action/receipt system. Startup can reject a valid
  non-default value such as `query.limit=11` because it treats startup as receipt-gated live reconfiguration.
- `PersistentControlJournal` appends a permanent operation receipt for every mutation, copies all records into
  a new `ControlSnapshot` and scans history during recovery. Completed bookkeeping becomes a lifetime write
  limit.
- Registry commands restore complete operation history and original input layouts before changing one
  current source.

### Decision

Use one current-format root manifest and one real OS guard. Construct owners directly from one bounded typed
settings value. Use short typed redb transactions for:

```text
AppliedSettings
RootRecord
SourceRecord / current RevisionRef
PreparedRef
CurrentPublishedView
PendingPublication?
RetiredCleanup[]
```

redb stores current references and finite unfinished external work, not source bodies, completed-operation
history, query history or caller proofs. Resolve an uncertain commit by reading the affected typed row.

## 5. Source acquisition, identity and reconciliation

### Findings

- `acquire_source_v2` reads, hashes and classifies a body, then returns an object which no longer owns those
  bytes. The command reports `retained_revision:false`.
- Root sync performs preflight checks and then ends in `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`.
- Source admission, receipt, provenance, records, history, identity and registry packages repeat policy,
  original input, operation and history semantics without producing one retained source head.
- Normal registry commands bind the v4 restore adapter and reconstruct full history for one mutation.
- Watcher/currentness types do not form a connected inventory/update process. A remove event alone cannot
  prove final absence after editor atomic-save behavior.

### Decision

```text
pre-read decision
→ one stable final-handle/Git read
→ same acquired bytes into one immutable writer
→ one source-head compare/set transaction
→ one preparation job
```

Use one `ignore::WalkBuilder` inventory and raw `notify::RecommendedWatcher` hints feeding one bounded dirty
queue. Incomplete scans never retire unseen files. Git views use exact OIDs, `ls-tree`, raw `diff-tree` and
one reused bounded `cat-file --batch-command`; no checkout/filter/process-per-file path.

Delete admission receipts, provenance archive, registry batch replay, original-input restoration and
forwarding source packages after their real consumers move.

The active private #598 source/object/key branch is outside this audit. Consume only its accepted handoff;
do not create a competing writer.

## 6. Immutable objects and storage protection

### Findings

Revision store and materializer contain separate immutable publication kernels. Both stage, write, sync,
install and read back objects. Their success paths can leave staging names. The current Windows directory
sync is effectively a no-op which still reports success; final readback proves current contents, not
sudden-power-loss durability.

The public secret/crypto stack combines unrelated provider pairing, Qdrant credentials, legacy DPAPI,
revision DEK selection, migration and generic secret lifecycle. `search-revision-crypto` exists without a
production caller while #598 owns the selected encrypted storage path.

### Decision

One object writer/reader:

```text
unique create-new stage
→ bounded write and file sync
→ no-clobber content-addressed finalization
→ exact final object verification
→ selected namespace-durability profile
→ remove stage
→ ObjectRef
```

The existing old cutover-marker publisher contains a useful finalization shape; extract the mechanism and
delete the migration owner. Plain storage invokes no key system. The encrypted profile is exactly the
accepted #598 result. `search-revision-crypto` survives only if that result directly uses it.

## 7. Preparation and incremental identity

### Findings

- Markdown/JATS verification can reopen the retained revision and rebuild the same materialization between
  adjacent stages.
- New preparation objects are read repeatedly after staging/finalization.
- UTF-8/text buffers and canonical UnitSet representations are copied/serialized for checks then discarded.
- Current `UnitId` includes full revision, absolute coordinates and ordinal. A small insertion can change the
  suffix of units.
- S11 v1 includes representation and current Unit ID, so revision/coordinate churn propagates into Qdrant
  point identity.

### Decision

Create one `MaterializationProduct`, pass it directly into structure-first unitization and persist one
prepared artifact through the same object store.

Source units are structural and model-independent: sections/items, code definitions/blocks, document
pages/items and a line-preferred plain fallback. Split only oversized structures; merge small adjacent peers
only under the same context. Model token packing is a derived projection.

**Do not preserve S11 v1.** Preserve only useful collision/address mechanics: closed typed preimage, full
identity digest, 128-bit Qdrant address and complete readback collision refusal. The new point identity binds
stable retrieval meaning:

```text
stable grouping / projection membership
occurrence incarnation
retrieval profile and role
exact retrieval input identity
point role
```

Exact source revision, representation, occurrence and range live in `ViewManifestEntry`. A later view can
reuse an unchanged retrieval point while retaining a different exact citation. Equal text occurrences remain
distinct.

## 8. Qdrant bridge and lexical search

### Findings

- `RealDataPlane` and `BlockingRealQueryPlane` perform real SDK operations, but normal daemon
  publication/query does not call them.
- Runtime construction requires a historical `QualifiedGate`, exact executable/client identity and probe
  outcomes plus an API-key equality lease. Those reports do not describe the service currently running.
- The bridge keeps a process-local operation receipt map. Restart loses it; successful calls consume a
  fixed lifetime capacity.
- `OpContext` holds a relative duration, allowing nested RPCs to renew the caller budget.
- Collection schema hard-codes a frozen sparse-only, one-shard, strict/strong profile.
- Qdrant payload duplicates citation revision/range and vector/payload digest fields. Query echoes those
  values as if they proved source bytes.
- Existing query accepts client-computed sparse pairs instead of native `Document` input for
  `qdrant/bm25`.
- Full upsert can erase unspecified named vectors; `set_payload` merges rather than replacing payload.

### Decision

One daemon-owned client/runtime serves publication and query. Connect from selected endpoint and actual
optional credential. Qualification is release/upgrade evidence, not a constructor token. Use one absolute
monotonic deadline/cancellation across every RPC. Publication owns unknown mutation recovery through actual
readback; the bridge owns no replay ledger.

Use native Qdrant mechanisms:

- `Document` input for stored/query `qdrant/bm25`;
- explicit base-population `params.idf.corpus` separate from query-only narrowing;
- full upsert only for complete authoritative replacement;
- selected vector update/delete;
- payload merge/delete/overwrite with exact semantics;
- `wait=true` and exact relevant readback;
- native prefetch/RRF/grouping for compatible Qdrant branches.

Local baseline is one node, one shard, one replica and one serialized publisher. Stronger topology/order is
an explicit measured profile. Incompatible schema/profile builds a new physical generation.

## 9. Publication, view lifetime and cleanup

### Findings

- Daemon Publisher, shared publication coordinator and control codecs duplicate publication state.
- A prepared generation may occupy in-memory coordinator state before durable intent and can become
  difficult to abandon.
- Current pending intent does not contain or reliably resolve the complete operation after restart.
- Route pins, epoch pins, watermarks, reclaimer proofs, retention leases and purge/restore systems overlap.
- A sampled pin/watermark cannot prevent a new old pin from appearing after the sample.

### Decision

```text
immutable operation/manifest pages
→ small PendingPublication row
→ bounded Qdrant effect/readback pages
→ one transaction:
     CurrentPublishedView = target
     RetiredCleanup += unfinished physical cleanup
     PendingPublication = none
→ short current-view pointer swap
→ cleanup after old guards drain
```

Queries can pin only the current immutable view. A query window clones an existing guard. After pointer
switch, no new old guard can appear; old guards only decrease.

Cleanup safety:

```text
point IDs closed at sequence N:
  no live same-route/generation view with sequence < N

old collection:
  no live view using that route/generation
```

Delete exact retired IDs or the old collection through the bridge and verify absence. Unknown deletion
leaks safely and retries. Generic retention, purge, restore quarantine and separate reclaimer operating
systems are removed from the local baseline.

Object GC derives roots from current source/prepared records, current view, pending publication, unfinished
cleanup and live view guards. Uncertainty retains bytes. A process-local GC gate under the one root owner
closes finalization-to-first-reference races; no sweep journal is required.

## 10. Query, evidence and paging

### Findings

The query stack contains recipes, capabilities, grants, public plans, dependency fingerprints, overlap
proofs, scheduler snapshots, leg tickets, mutable terminal maps, relative deadlines, unconditional exact
count, caller-shaped readback proofs, a raw-score re-sorting projector and four handle/continuation stores.
Most principal production APIs are reached from fixtures/process tests rather than normal serving.

Specific defects:

- candidate IDs include leg ID, preventing fusion of the same point across legs;
- projector can destroy final fusion order by re-sorting raw scores;
- candidate validator compares digest fields supplied next to bytes but does not establish those bytes;
- page/expand paths repeatedly enumerate and hash the complete source catalogue;
- shared continuation can rescan the issued prefix;
- handle expansion accepts arbitrary ranges in an entire revision;
- ordinary top-k calls exact count and carries completeness machinery it cannot justify.

### Decision

Direct current operations:

```text
Search
Exact
Continue
Expand
```

`Search`:

```text
pin PublishedView
→ non-widening ResolvedScope
→ one RequestContext(deadline, cancellation, finite counters)
→ real Qdrant BM25/native query
→ preserve final stable point order
→ pinned ViewManifestEntry lookup
→ select page before source I/O
→ actual retained object/range verification
→ EvidenceCard[]
```

A Qdrant hit is a nomination. Source evidence is the manifest plus immutable object/range verification.
Absent/foreign/conflicting points contaminate the backend result; valid entries with unavailable evidence
become gaps. Echoed digests are not proof.

One process-local `QueryWindow` stores final order, cursor, view guard, expiry and finite limits. Cards carry
source/revision/location/excerpt directly. Optional `WindowItemRef` expands only a bounded neighborhood of the
exact emitted item. Window insertion/cursor advancement happens only after a complete response frame.

`Exact` streams one ordered pinned inventory, reads/verifies/releases one object at a time and reports
truthful complete/incomplete outcome. Keep the useful Aho-Corasick matcher and at most one bounded
`regex-automata` adapter; delete exact proof/checkpoint/denominator machinery.

## 11. Protocol, client and CLI

### Findings

Provider protocol/client/daemon stacks combine manual fixed-order JSON/base64, pairing challenge/MAC,
binding lifecycle, grants, capability descriptors, eleven V1 recipes, parallel V2 evidence schemas and
session/replay state. A separate public runtime uses tab/hex lines. The active CLI's typed-looking indexed
request is translated back into the legacy line protocol.

### Decision

One wire:

```text
[u32 little-endian length][bounded UTF-8 Serde JSON]
```

Current messages carry `Search | Exact | Continue | Expand`, request ID, finite timeout and one terminal
response with optional bounded progress. Check declared size before allocation. Encode and size-check the
complete output before first write. Partial output is fail-stop and commits no window state.

Trusted baseline uses child stdin/stdout or an owner-only local pipe. It needs no pairing key, per-frame MAC,
product grant or capability digest. A real restricted external profile authenticates separately and enters
the same dispatcher.

CLI captures `args_os()` once, owns child handles, builds the current request directly, uses one deadline,
sends best-effort cancel and performs bounded child cleanup. Remove legacy core/TCP/token/endpoint/tab/hex
paths and duplicate clients.

## 12. Shared contracts, ports and domain

### Findings

`search-contracts` became a catalogue of planned recipes, plans, grants, proof records, lifecycle types,
indexed vendor-like schemas and several result families. `search-ports` defines 23 global traits and about 80
operations; many have no real production implementation. `search-domain` centralizes rules whose actual
owners are source, publication, query or evidence.

### Decision

Keep only immutable data used by at least two real current owners: source/revision/object/view/point IDs,
locations, selected scope, finite limits, current request/result DTOs, retrieval profile semantics and
source-backed cards/gaps.

Use concrete owner methods by default. A narrow trait belongs beside a consumer only when multiple real
implementations exist. There is no global service locator, effect bus or conformance fake catalogue.
Concrete Qdrant/redb/OS/window/session types remain private to their owners.

## 13. Tests, tooling and generated governance

### Findings

The repository contains large families of module/file/function-name/line-ceiling tests, process harnesses
for disconnected applications, generated package maps, architecture coverage, accepted evidence digests,
P00 ticket/lease/handoff control records and a per-site digest approval ledger.

These systems freeze the package layout and mechanisms now being deleted. The integration bootstrap even
requires exactly 45 packages, fixed wave profiles, `CONTROL_JOURNAL_ONLY`, migration quarantine and paired
backup assumptions. The custom Qdrant boundary tool partially parses Rust and compares frozen
qualification/version records.

### Decision

Delete generated governance and source-shape policy with their consumers. Preserve causal behavior tests:
retained bytes/coordinates, integrity, ABA content reuse, bounded output, publication recovery, old-view
survival and the real product smoke. Use Cargo metadata, Rust privacy/compiler errors, direct API review and
actual live behavior instead of another source analyser.

During refactor use minimal changed-owner check and Clippy plus focused catastrophic semantics. Full
workspace/all-features/all-tests are late integration/release work, not a gate for every deletion slice.

## 14. Optional features and donors

Model worker and document worker are refusal scaffolds, not capabilities. Delete the model stub. PDF support
ships one selected profile or the stub is removed until implementation. Overlay, MCP, code intelligence,
orientation/comparison and evaluation remain thin optional consumers after standalone Search works.

Accepted donor seams:

| Need | Reuse |
|---|---|
| CLI dispatch | ripgrep parse-once direct dispatch |
| Inventory | `ignore::WalkBuilder` plus raw `notify` hints |
| Git | `ls-tree`, raw `diff-tree`, reused `cat-file --batch-command` |
| Control | redb typed transactions and MVCC reads |
| Serialization | bounded Serde JSON/TOML |
| Deadlines | one absolute remaining-time context / Tokio `timeout_at` |
| Qdrant | pinned client, native BM25/IDF/fusion/grouping and exact mutations |
| Structure | existing parsers plus selected Docling/Unstructured boundary ideas |
| View lifetime | Lucene snapshot refcount principle, not Lucene backend |
| Object GC | Tantivy living-file derivation principle, not Tantivy backend |
| PDF | one selected PDFium integration when #227 is implemented |

EdgeQuake, LightRAG, GraphRAG, NotebookLM, Cloudflare AI Search and Anthropic contextual retrieval supplied
specific lessons about early selection, context reuse and cost. They are not frameworks to import.

## 15. Implementation order

1. #330/#270 — one immutable writer/reader and one current source head using #598's accepted handoff.
2. #331 — one materialization, structure-first units and prepared reference.
3. #259/#329 — stable point identity and versioned manifest evidence.
4. #263 — one real Qdrant runtime and native BM25 path.
5. #261/#300/#303 — pending publication, committed current view and finite cleanup.
6. #302 — known-root object GC.
7. #272/#291/#293/#279 — selected scope, request context, evidence lookup and connected Search.
8. #275/#280/#332 — QueryWindow, framed protocol and actual CLI.
9. #249 — direct exact scan.
10. Delete superseded packages/features/tests in each cutover; do not postpone deletion to one final mega-task.

## 16. What this audit does not prove

- It is not a line-by-line audit of all 1,593 Rust files.
- It did not inspect private #598 worktrees.
- It did not compile or run the refactored design.
- It did not execute Windows durability, live Qdrant, crash recovery, scale or performance cases.
- It does not prove every optional package has been exhaustively reviewed.
- It does not claim that the documentation branch itself is fully reconciled; use the coverage map checklist.

The audit is complete only when every workspace member and project-wide document has a disposition, current
package instructions contain no conflicting requirements and remaining unreviewed areas are named. Until
then, describe it as a broad source-traced refactor audit—not a complete line-by-line verification.

# ELIOT Search — implementation

Read the [manifesto](../architecture/README.md) for product intent. This guide selects mechanisms and their
owners. Package notes contain the source seams; audit maps contain evidence and remaining uncertainty.
Names in sketches are descriptive, not mandatory new APIs/packages. Implement only the part consumed by the
current connected change. Keep correct existing code rather than rewriting everything labelled KEEP/REWRITE.

**Source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.
**Documentation re-review:** 2026-10-21, starting at `36fe7f4`. The earlier blanket audit-closure claim is
withdrawn; see the [coverage map](../audit/AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md).

## 1. Build one application

```text
selected source → retained bytes → current source head → prepared units
→ real Qdrant publication → published view
→ scoped query → manifest lookup → retained excerpt
```

One-file lexical search is the first executable milestone, not completion of the manifesto's large-corpus,
navigation, comparison and incremental-update requirements. A deferred implementation has no empty scaffold
or mandatory dependency, but its product requirement must not disappear merely because a package is deleted.

## 2. Owners and cutovers

| Responsibility | Source map | Change |
|---|---|---|
| Root and settings | [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [config](../../crates/search-config/FUNCTIONS.md) | Actual lifetime lock; typed settings; no owner epochs or activation receipts. |
| Source and objects | [source](../../crates/search-source/README.md), [objects](../../crates/search-source/search-revision-store/FUNCTIONS.md) | One stable read, one immutable writer, one current source head. |
| Preparation | [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [units](../../crates/search-prep/search-unitizer/FUNCTIONS.md) | One prepared product, useful structure and exact coordinates. |
| Index and publication | [planner](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md), [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) | Bounded pages, real effects, current view and unfinished cleanup. |
| Query and output | [query](../../crates/search-query/README.md), [windows](../../crates/search-query/search-continuation/FUNCTIONS.md) | Actual scope, one request context, verified excerpts and session-owned paging. |
| Local interface | [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) | One typed exchange with an explicit process/session lifetime. |

An owner is a responsibility, not necessarily a crate or service. Delete wrappers whose only purpose is to
preserve a discarded interface. Candidate donor APIs are choices, not a checklist of features to implement.

## 3. Current data

redb owns current roots, source heads, prepared references, the published view, one pending publication and
unfinished cleanup. It does not store bodies, vectors, query history or completed-operation archives.

The immutable store owns source/prepared/manifest/operation bytes. Qdrant owns derived retrieval state.
A view manifest maps stable points to exact retained revisions, representations and ranges. These are data
relationships; do not generate a separate repository, schema registry and serializer for every noun.

## 4. Source and control path

Capture invocation/config once. Inspect without repair; initialize explicitly. Hold the real root lock for
the process lifetime and use short typed redb transactions outside filesystem/Qdrant work.

Use one ignore-filtered inventory and raw watcher hints feeding the same dirty queue. Incomplete inventory
never retires unseen sources. For a selected Git view, resolve its exact OIDs once and use bounded raw Git
plumbing; no checkout/filter/hook/fetch pipeline is implied.

```text
small pre-read policy → stable final-handle read → post-read classification
→ same acquired bytes into object writer → current-head compare/set → preparation
```

Do not reread the mutable pathname between acquisition and persistence. Finalize the object before its first
durable reference; unknown commit resolves by reading the affected row. Remove registry replay, original-input
archives and admission receipts with callers. The object writer owns actual no-clobber/sync/readback behavior;
Windows namespace durability is unverified until the selected primitive is exercised, not established by a
receipt or final readback alone.

#598 owns overlapping private object/key work. Preserve that work and coordinate its public handoff; unrelated
query, protocol, config and documentation changes need not wait for it. Its existing public issue still
contains six-domain, purge, restore-quarantine and old-format obligations; those do not transfer to this
architecture. Integrate useful accepted code, not its superseded programme.

## 5. Preparation and point projection

Pass the same owned materialization into unit construction. Validate loaded external/persisted artifacts at
the boundary, not by repeating the producer pipeline between adjacent stages.

Use existing Markdown/JATS structure and line-preferred plain-text fallback. Code definitions require a real
selected syntax producer; do not fabricate them. Split oversized items locally. Token/model packing is a
projection, not source identity.

Separate occurrence identity, current source location and actual retrieval input. A changed offset alone
must not force unrelated point rewrites. Equal text is not proof that two occurrences are identical. Matching
ambiguous duplicate/moved structures needs a bounded rule; conservatively recreate ambiguous occurrences
rather than inventing a universal lineage engine or claiming all edits preserve IDs.

Projection carries uploadable text/vectors plus target evidence bindings in bounded pages. Page ordering and
point lookup need a concrete implementation; 'paged' alone does not prove bounded memory or fast lookup.
Avoid complete old/new manifest copies and a new generic plan/fingerprint framework.

## 6. Qdrant

Use one concrete client/runtime for the selected endpoint and actual credential. Remove qualification tokens,
per-RPC key-equality leases and the process-local mutation receipt ledger.

Native BM25 document/query inputs and explicit base IDF population replace the local sparse engine. Preserve
Qdrant order for baseline search. Add native hybrid/prefetch/grouping only when a real selected branch uses
it. Implement only mutation methods needed by current publication; the vendor's API catalogue is not a task
list. Full upsert replaces a point; vector/payload patches must preserve unrelated state.

Every RPC consumes the operation's remaining deadline. Mutation uncertainty belongs to publication and actual
readback. A partially applied batch is not automatically corruption: distinguish expected pre-state, desired
post-state and genuinely foreign state per affected item before retrying idempotent work. Incompatible
profile/schema builds a new generation; no relabelling old points.

## 7. Publication and lifetime

Keep one publication owner:

```text
immutable operation/manifest pages → PendingPublication
→ Qdrant apply/readback → redb transaction(current view + unfinished cleanup - pending)
→ short in-process current-view install → eligible cleanup
```

The durable commit precedes pointer installation. Never hold the view-manager mutex over disk/RPC I/O. A
request pinning the old view in that gap remains valid. Recovery reads the actual current/pending/cleanup
rows, not historical success receipts.

New requests pin only current. Existing requests may keep their guard in a live query window. For point
closures at N, wait for every same-generation live view below N, not only the immediate predecessor. A
collection waits for all its users. Cleanup rows contain only unfinished effects and disappear on completion.
Use ordinary Arc/Weak ownership; no public pin IDs, renewal or acquisition of old views by ID.

## 8. Immutable-object GC

Protect current source/prepared refs, current/pending/cleanup manifests and live views. A shared producer guard
must cover finalization/reuse through first durable reference; GC must not delete in that gap. Keep filesystem
I/O out of the view-manager lock. Unknown/corrupt/incomplete root information retains bytes.

The current GC proposal still needs a bounded traversal/cost analysis: rescanning every live manifest for
every deletion page is not made cheap by calling each page bounded. Settle this in the object/publication
owner without reinstating generic leases, a graph database or permanent sweep history. No claim of completed
GC design or runtime verification is made here.

## 9. Query and evidence

A handler pins the view, resolves the selected non-widening scope and owns one request deadline, cancellation
and finite nomination/read/output counters. Baseline Search calls Qdrant directly; no plan/capability/grant
objects are needed. Exact streams the selected retained inventory and alone may claim complete no-match.

Select a page before retained-source reads. Lookup each point in the pinned manifest, read actual retained
bytes and preserve final rank through formatting. Foreign/conflicting manifest points fail that backend
result; missing retained evidence is an explicit gap. Hash echoes do not establish bytes.

### QueryWindow

The [window owner](../../crates/search-query/search-continuation/FUNCTIONS.md) specifies lifetime, page
accounting and output commit. The window belongs to one live session; a one-shot CLI must not print a locator
which will die as soon as its child exits. Each Continue/Expand has a fresh request deadline capped by the
unchanged window expiry. Never reuse the completed Search deadline or renew the window TTL.

Advance only across candidate references whose card or gap was represented in the successfully written
response. Output-limited, unprocessed references remain for the next page. A successful flush is not proof of
client receipt; lost/partial sessions expire their windows rather than requiring exactly-once delivery.

## 10. Local protocol and CLI

Use one length-prefixed typed JSON exchange. The [protocol map](../../crates/search-provider-protocol/FUNCTIONS.md)
owns the single wire/I/O recipe; package READMEs link to it instead of copying it.

Separate a one-shot command from a multi-request session using the same transport and dispatcher. One-shot
returns no live locators after exit. Session mode keeps the owned child until explicit session end, disconnect,
expiry policy or failure; a terminal event ends a request, not the session. Persistent cross-invocation daemon
support is a separate selected need, not a prerequisite or an excuse to persist windows.

One active data request still requires a control reader able to receive Cancel/EOF while the handler runs.
Do not advertise cancellation with a read→execute→read loop. Use the existing runtime and a small bounded
reader/control path, not a general request scheduler. Check actual blocked-I/O behavior: polling a clock
before blocking Write, or aborting a started spawn_blocking task, cannot guarantee interruption.

Encode with Serde into a capped buffer before any transport write. `to_vec` followed by a size check does not
bound serialization allocation. Keep one response writer; on partial I/O close the session and commit no
page progress. Transport/cleanup guarantees must match executed platform behavior.

## 11. Optional depth

Delete dead scaffolds, not the purpose of the product. Ordinary path/heading/range information supplies the
first orientation. More precise outline/SCIP, selected document extraction, dense retrieval, reranking,
overlays, remote clients and MCP are separate changes only when requested with a real consumer. An unselected
option adds no enum variant, provider registry, worker, config section or dependency now.

The manifesto still owns navigation/comparison/large-corpus goals. Removing an existing optional package
means its current design is not reused; it does not silently cancel the user's goal or require a model.

## 12. Donor seams

Use the [donor register](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) for inspected behavior/version
limits: redb transactions; Qdrant retrieval/mutations; Serde/TOML; ignore/notify; Git plumbing; selected parsers
and matchers. Borrow only the needed mechanism. No donor becomes a second backend or framework.

For framing specifically, Serde `to_writer` supports a capped local Write sink. Tokio blocking-I/O cancellation
limitations are recorded in the protocol map. Do not turn an upstream limitation into a false timeout promise.

## 13. Implementation order

Integrate one retained-source path with #598's coordinated handoff (#330/#270), preparation (#331), stable
retrieval/evidence (#259/#329), real Qdrant (#263), publication/view/cleanup (#261/#300/#303), source-backed
query (#272/#291/#293/#279) and CLI/session (#275/#280/#332). Exact and rebuild use the same owners.

This is data dependency order, not a global writer queue. Independent code and cleanup can proceed on actual
non-overlapping responsibilities. Do not require a full product in each small change or create missing
prerequisite frameworks. The first end-to-end milestone may span several compiling changes; remove each
replaced path when its real callers move.

## 14. Verification

Write the selected code and run minimal pinned check/Clippy over changed owners/consumers. Focused causal
checks follow the changed behavior; broad test/native/recovery/scale work follows the connected product.
No audit map, package count, review badge or old byte fixture is a prerequisite for independent implementation.

Still required as behavior exists: retained restart; actual CLI→Qdrant→excerpt; edit/rename reuse; selected
IDF/scope; old-view lifetime; pending/current/cleanup crash boundaries; first-reference/GC race; session
paging; short output; blocked-I/O cancellation; index rebuild and truthful unsupported modes. Record unrun
cases explicitly. Source inspection and this guide establish no runtime or exhaustive-audit guarantee.

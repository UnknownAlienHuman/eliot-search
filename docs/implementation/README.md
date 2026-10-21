# ELIOT Search — implementation

Read the [manifesto](../architecture/README.md) for purpose. This document selects mechanisms; local FUNCTIONS
maps contain their single operation recipes. Examples are not orders to create a type, trait, service or
package for every noun. Keep correct existing code. Audit maps are evidence, not permission to code.

Source baseline is `c8e38b6`. This is refactor preparation, not implemented or runtime-qualified behavior.
A one-file lexical result is the first milestone, not completion of navigation, comparison, documents,
incrementality or large-corpus requirements.

## Connected path

```text
selected source -> stable read -> retained bytes -> current source head -> prepared units
-> real Qdrant effects -> atomic published evidence/view
-> scoped query -> snapshot point lookup -> actual retained excerpt
```

## Owners

| Operation | Single recipe |
|---|---|
| Root/config | [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [config](../../crates/search-config/FUNCTIONS.md) |
| Source/object | [source](../../crates/search-source/README.md), [store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Preparation/occurrence matching | [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [units](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Projection/evidence lookup | [planner](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md), [control](../../crates/search-control-redb/FUNCTIONS.md) |
| Effects/view/cleanup | [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Local object collection | [stopped-root maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |
| Query/window/session | [query](../../crates/search-query/README.md), [window](../../crates/search-query/search-continuation/FUNCTIONS.md), [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |

An owner is a responsibility, not a package count. Change real consumers with their producer and remove the
replaced mechanism. Do not implement every optional donor method or preserve a dead API through a facade.

## Source and preparation

Capture args_os/config once. Inspect without repair; initialize explicitly; hold one real root lock. Use one
ignore-filtered inventory plus raw watcher hints. Incomplete scans never mass-retire unseen sources. Selected
Git views use exact OIDs and bounded raw plumbing, not checkout/hooks/filters or implicit network.

Transfer the exact acquired body into the one immutable writer before a short source-head compare/set.
Resolve unknown commits from actual rows, not operation archives. Use one materialization and one prepared
artifact; no reparsing between adjacent stages or second writer. Keep exact source ranges separate from
retrieval prefixes and model packing.

Unchanged input/profile reuses the existing product. For edited sources, preserve occurrence IDs only for
unambiguous one-old/one-new matches. Duplicate positions are not stable anchors; ambiguous repeats receive
new births. This knowingly sacrifices some reuse rather than building a lineage engine. Measure rewritten
points, not just cache hits. The unitizer map defines the bounded matching rule.

## Evidence metadata and lookup

Use redb 2.6.3, already pinned by the project, for compact published source/evidence rows. One generation's
evidence keys are (source_id, point_id); its source table records the exact published inventory. The view
holds a real redb read snapshot. A later query page must not read current metadata under an old view label.

This replaces the undefined immutable manifest page-index proposal. No custom B-tree, full manifest copy,
per-view table copy or generic temporal database is needed. Bodies, vectors and operation-page payloads stay
in the immutable store/Qdrant. A ViewManifestEntry is the compact evidence row, not a second authority.

Ordinary source-local metadata changes commit atomically with current view after Qdrant readback, within a
prechecked delta ceiling. A full/profile rebuild fills separate generation tables in bounded transactions
and switches a pointer only after completion. The control and publication maps specify the two cases.

Read snapshots retain old redb pages; finite windows and write backpressure are required. Database growth and
latency must be measured with old readers held. 'Snapshot' and 'bounded' are not cost-free guarantees.

## Publication and retrieval

One concrete Qdrant client/runtime serves native BM25 and exact required mutations. The selected base
population supplies IDF; query-only constraints do not silently redefine it. Preserve server rank. Add
hybrid/rerank branches only for real selected consumers. Full upsert and narrow updates have different
semantics; implement only the operations consumed now.

A pending artifact fixes actual inputs and readback targets. After partial or uncertain application, desired
items are complete, pre-state items are retried, and foreign state blocks publication. Keep the same pending
operation; no second bridge receipt ledger. Commit evidence, source inventory, current pointer and unfinished
cleanup atomically. Capture/install the matching read snapshot before reporting visible or starting the next
publication. No manager lock across disk/network work.

Requests acquire only current. Old guards retain their exact database snapshot and Qdrant sequence. Point
cleanup waits for every live same-generation sequence below the closure threshold; collection cleanup waits
for all its users. Durable cleanup contains only unfinished effects, not completed history.

## Local object lifetime

The selected first collector runs only as explicit maintenance after the daemon releases its root lock.
It marks all current source/prepared, published, pending and cleanup references once, then enumerates/deletes
unreferenced objects. A disposable redb mark table bounds RAM without writing a new graph/sort framework.

This deliberately replaces the online per-page GC gate proposal: rescanning all roots per deletion page has
unbounded pause cost relative to that page. There are no live producers/windows during maintenance, so no
finalization-to-first-reference race exists. Normal writes still remove their own successful stages. Disk
pressure pauses new writes and reports maintenance; it does not silently delete evidence. Qdrant cleanup
remains online and independent. Online local GC is deferred, not advertised as implemented.

## Queries, windows and process lifetime

Search pins a view, resolves non-widening scope, and owns one deadline/cancellation with finite read/output
limits. Exact alone may claim complete no-match over its actually scanned published inventory. Select a page
before source reads; resolve its metadata by exact key; verify actual retained bytes; preserve rank and gaps.

A window belongs to one live session. One-shot CLI output contains no locator whose child is immediately
destroyed. Continue/Expand use new request deadlines capped by fixed window expiry, not the expired Search
deadline. Commit only the handled candidate prefix represented by successful output. Local flush is not peer
acknowledgement; lost sessions do not require exactly-once receipts.

One framed typed protocol serves both modes. Cancel/EOF must remain observable while a handler runs. A clock
check before blocking I/O does not interrupt it; use actual platform cancellation or owned-child termination
and measure the bound. Serialize into a capped buffer before transport output. Partial output fails the
session and advances no cursor. No local pairing/grant/capability framework is required.

## Optional depth and coordination

Navigation/comparison/documents remain product goals; deleting broken scaffolds does not cancel them.
Precise syntax/SCIP, selected extractors, semantic models, overlays and MCP are implemented only with a real
consumer. Unselected options add no worker, provider registry or configuration section now.

Protect #598's actual private object/key overlap and consume a coordinated public handoff. Do not inherit its
retired six-domain/purge/restore/compatibility requirements or block unrelated work. One manager-owned
worktree; existing issues and ordinary commits identify work, not another audit-control system.

## Order and verification

Integrate retained source/head/preparation, identity and real Qdrant, atomic evidence/publication, query and
CLI/session. Independent non-overlapping changes need not wait for a full product. Use minimal pinned
check/Clippy after code; runtime/native/fault/scale checks follow real implementation. Keep unsupported old
roots and sole retained user data intact. Historical byte/shape/qualification gates do not direct the rewrite.

Unrun checks remain explicit: retained restart; lookup without corpus scan; duplicate edits; old snapshots;
bounded metadata commit; partial RPC recovery; rebuild staging; stopped-root GC; paging/truncation/cancel;
Windows I/O/durability and actual retrieval quality/large-corpus costs. No exhaustive audit closure is claimed.

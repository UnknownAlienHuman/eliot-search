# ELIOT Search — implementation

Read the [manifesto](../architecture/README.md) for purpose. This guide selects mechanisms; local FUNCTIONS
maps own operation recipes. Examples are not instructions to create a type, trait, service or package for
every noun. Keep correct existing code. Audit records are evidence, not permission to code.

Source baseline: c8e38b6. This is proposed refactor behavior, not build/runtime qualification. One-file lexical
search is the first milestone, not completion of navigation, comparison, documents or large-corpus goals.

## Connected path

```text
selected source -> stable read -> retained bytes -> current source head -> prepared units
-> real Qdrant effects -> atomic published evidence/view
-> scoped query -> snapshot point lookup -> actual retained excerpt
```

An arrow is not a recovery mechanism. Durable input/outcome comparison must recover lost wake-ups; a stale
completion cannot overwrite current state. Restart, restored copy, supported empty and failed work are
separate cases, not labels hidden behind the happy path.

## Owners

| Operation | Single recipe |
|---|---|
| Root/config | [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md), [config](../../crates/search-config/FUNCTIONS.md) |
| Source/object | [source](../../crates/search-source/README.md), [store](../../crates/search-source/search-revision-store/FUNCTIONS.md) |
| Preparation/reconciliation | [materializer](../../crates/search-prep/search-materializer/FUNCTIONS.md), [unit matching](../../crates/search-prep/search-unitizer/FUNCTIONS.md) |
| Projection/evidence lookup | [planner](../../crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md), [control](../../crates/search-control-redb/FUNCTIONS.md) |
| Effects/view/cleanup | [publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md), [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Local collection and backup/restore | [stopped-root maintenance](../../crates/search-runtime/search-retention/FUNCTIONS.md) |
| Query/window/session | [query](../../crates/search-query/README.md), [window](../../crates/search-query/search-continuation/FUNCTIONS.md), [protocol](../../crates/search-provider-protocol/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |

An owner is a responsibility, not a package count. Change real consumers with their producer, then remove
replaced mechanisms. Do not implement a donor's API catalogue or preserve a dead interface through a facade.

## Source and preparation

Capture args_os/config once. Inspect without repair, initialize explicitly and hold one real root lock.
One ignore-filtered inventory plus raw watcher hints serves updates. Incomplete scans never retire unseen
sources. Git uses exact OIDs and bounded raw plumbing, not checkout/hooks/filters or implicit network.

Retain the exact acquired bytes before source-head compare/set. Unknown commit resolves from current rows.
One materialization feeds one prepared artifact through the shared object writer; no adjacent-stage reparse.
Source ranges, retrieval prefixes and model packing are distinct.

The preparation recipe compares current wanted input with its current outcome after startup, lost queue
work or profile change. Queue entries are disposable. Worker completion conditionally installs only a still-
current input; stale work cannot revive a removed source. Permanent gaps are recorded, not busy-retried.
Publication similarly derives unpublished work from current preparation versus published inventory.

A supported complete empty input yields zero units, not a fake unit or failure. Unsupported/truncated input
is an explicit gap, never proof of empty content. Mapped/lossy text can be useful without claiming raw-byte
identity or completeness. A nonempty -> empty update removes former current points through publication.

For edited sources reuse occurrences only under the unitizer's one-old/one-new rule. Duplicate position is
not continuity; ambiguous repeats receive operation-fixed new births. Measure actual rewritten points, not
just cache hits. Pure transform reuse does not resurrect a closed point interval.

## Evidence metadata and lookup

Use pinned redb 2.6.3 for compact published source/evidence rows. One generation's evidence keys are
(source_id, point_id); its source table records the actual published inventory. Views retain real read
snapshots, never old sequence labels read against latest coordinates.

No custom manifest B-tree, per-view table copy or temporal database. Bodies, vectors and operation payloads
stay in the immutable store/Qdrant. Ordinary bounded complete-source metadata deltas commit atomically with
the current view. Full/profile rebuilds fill separate generation tables in bounded transactions, then switch
a pointer. Long readers retain pages; finite windows and measured update backpressure matter.

## Publication and retrieval

One concrete Qdrant runtime serves native BM25 and only currently consumed mutations. The selected base
population supplies IDF; query-only filters do not redefine it. Preserve server rank. Optional hybrid/rerank
and unused vector/payload wrappers are not baseline work merely because the SDK supports them.

A redb snapshot does not freeze Qdrant. Published point vectors and query-visible retrieval/filter/group
state are immutable; changed meaning creates another point or generation. Only the fixed future validity
closure may change in place. Citation-only changes stay in the evidence rows. The bridge recipe distinguishes
known-vector comparison from native Document generated-vector readback; no second local BM25 encoder or
payload digest can supply independent proof of server inference.

Pending input fixes operation, births and targets. First distinguish observed point state from whether an
old dispatched writer may still execute. Desired/pre-state readback resolves ordinary retry only after the
relevant dispatches settle. A timeout, reconnect, local single publisher or strong ordering alone is not a
fence against late writes. Preserve possible dispatch across restart; do not blindly resend or clear pending.
The publication recipe chooses bounded isolated-generation recovery when terminality cannot be established,
retaining the abandoned route's unfinished obligation. This can cost a rebuild and retained garbage, not a
new receipt/recovery platform. Each attempt has a finite budget; nested calls cannot renew it.

Commit evidence, published inventory, current view and unfinished cleanup atomically. Capture/install the
matching read snapshot before visibility or another publication. No manager lock across disk/network I/O.
New requests pin current only; old guards retain their exact snapshot and Qdrant sequence. Point cleanup
waits for every earlier relevant live view, collection cleanup for all users, and both account for outstanding
writers that could recreate the retired state. Cleanup is unfinished work, not permanent history. Physical
generation names are not reused. Valid zero-point views are not missing/failed generations.

## Local object lifetime and restore

Local final-object collection is explicit stopped-root maintenance. Mark all current/prepared/published/
pending/cleanup references once using disposable scratch redb data; complete validation before sweeping.
No active producer/window exists during it. Serving removes successful stages but retains final objects;
disk pressure pauses new writes and reports maintenance. Qdrant cleanup remains separately online.

Backup creates an archive, not a second runnable root. Restore explicitly imports supported committed
source/prepared state into a fresh destination and Qdrant generation. It must not run copied pending or
cleanup against the original collection. A local lock in the copy does not protect the original external
index. Keep the original/archive intact and never substitute newer live-path bytes for retained evidence.

Protected storage adds a real constraint, not a general security framework: readable copied ciphertext does
not authorize writing with a copied/rolled-back key/nonce counter. Require the selected owner's safe independent
destination write context or report that restore mode unsupported. No automatic key reset, relabel or downgrade.
See the maintenance recipe; the old six-domain/quarantine programme is not reinstated.

## Queries, windows and process lifetime

Search pins a view, resolves non-widening scope and owns one request deadline/cancellation and finite read/
output limits. Exact alone may claim complete no-match over its actually evaluated inventory. Select the
page before source I/O, lookup exact metadata, verify retained bytes, preserve rank and report gaps.
Known empty selection/inventory returns empty without backend RPC; unknown IDs and missing/incomplete state
do not. An empty set must never turn into an omitted wildcard filter.

One window belongs to one live session. One-shot output cannot expose a locator destroyed on child exit.
Continue/Expand get new request deadlines capped by fixed window expiry. Commit only the handled prefix
represented by successful output. Local flush is not peer acknowledgement; lost sessions need no durable
exactly-once system.

The control reader observes Cancel/EOF during active work. A pre-I/O clock check does not interrupt blocking
I/O; implement actual cancellation/owned-child termination and verify the platform bound. Serialize into a
capped buffer before writing. Partial output fails the session and advances no cursor. Trusted local use
needs no pairing/grant/capability framework.

## Optional depth and coordination

Navigation/comparison/documents remain goals; deleting scaffolds does not cancel them. Precise syntax/SCIP,
extractors, models, overlays and MCP need a real consumer before adding workers, registries or dependencies.

Protect #598's actual private object/key overlap and coordinate its public handoff. Do not inherit its obsolete
six-domain/purge/compatibility programme or block unrelated work; do retain actual cipher/storage correctness.
One manager-owned worktree and existing issues/commits suffice. No second audit-control system.

## Order and evidence

Integrate retained source/head/preparation, identity/Qdrant, atomic publication, query and CLI/session.
Independent changes need not wait for the whole product. Minimal pinned compiler/Clippy follows code;
causal/native/recovery/scale runs follow the implementation they exercise. Unsupported roots and sole retained
copies remain intact. Historical bytes/shapes/qualification gates do not direct the rewrite.

Unrun cases include lost wake-up, stale completion, empty/gap transitions, same-root restart versus restored
copy, encrypted counter continuity, exact lookup, duplicate edits, old-reader growth, partial and delayed RPCs,
immutable old-view retrieval, native inference/readback, isolated-generation recovery, staging, stopped-root
collection, blocked I/O and actual retrieval quality/scale. The
[audit evidence](../audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md#18-qdrant-lifetime-late-writes-and-native-inference-readback)
records counterexamples; it does not certify that every defect has been found.

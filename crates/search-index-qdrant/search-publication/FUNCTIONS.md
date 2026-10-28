# Publication — complete generations before measured incrementality

One serialized publisher owns fixed build input, external effects, matching durable evidence/current state, view
installation and finite cleanup obligations. It owns no reader/parser/query window or second recovery platform.

<a id="reconcile-desired-and-published-state"></a>
## Desired state

Compare both the selected current source/preparation snapshot and published inventory; a success-only scan misses
removed or newly failed sources. Use bounded ordered reads. Startup/explicit/periodic reconciliation repairs lost
queue hints. Repeated identical outcomes are no-ops.

| Observation | Desired outcome |
|---|---|
| Complete usable preparation, including zero units | Publish exact captured input and its gaps. |
| Selected terminal unsupported/failure | Gap-only outcome; never relabel old evidence as the failed revision. |
| Confirmed removal/exclusion after complete inventory | Omit membership in the new view even without a new prepared artifact. |
| Running/incomplete/unavailable observation | Do not infer removal or empty success; retain prior served data explicitly labelled prior/pending where applicable. |
| Exact desired/published match | No new generation. |

The source owner distinguishes confirmed absence from filtered/incomplete inventory. Desired input and served
published input remain different facts. Changes arriving during a fixed build become later work.

## Fixed pending generation input

[Control](../../search-control-redb/FUNCTIONS.md#restartable-generation-input-capture) owns complete capture and
staging evidence tables. Before backend effects, persist one bounded PendingGenerationBuild referencing exact
source/prepared objects/outcomes, physical profile, never-reused collection name and complete uploadable pages.
Each page contains actual retrieval text/payload and fixed point IDs with finite byte/count limits.

An in-memory read transaction is not a durable manifest. Interrupted incomplete capture discards the metadata-only
target; complete capture never substitutes newer live rows. Oversize input refuses or produces an explicit selected
gap, not a falsely complete half-source. Retry preserves the same fixed input.

## Stage 2/3A complete-generation operation

```text
fixed input/pages → exact collection and required indexes
→ InsertOnly batches with wait → exact schema/point/payload/count readback + TextQuery readiness canary
→ finish bounded staging evidence/inventory writes
→ short compare/commit: current pointer + predecessor cleanup + clear pending
→ open/verify matching redb snapshot → short PublishedView install → report visible
```

The final pointer transaction does not copy corpus-sized evidence rows; they were built in bounded transactions
while the target was unserved. No redb writer or view mutex spans object/backend I/O. Do not start another
publication before view installation succeeds or recovery refuses serving.

Query addresses the view's physical collection, not an alias. Visible Stage 2/3A collections are immutable.
A physical profile or selected-state change builds a fresh complete generation. Query k/b alone does not.

<a id="apply-observed-state-is-not-dispatch-completion"></a>
## Uncertain effects

An attempt uses one absolute execution deadline/cancellation. A lost response, timeout, client drop or process
restart is not rollback or remote termination. Exact readback observes state; it does not settle every earlier
possibly dispatched writer. Preserve bounded unresolved progress with the pending build, not a separate ledger.

A completed mutation followed by failed readback retries observation first. Do not publish or clean a target while
an unresolved attempt could still change the required state. Identical InsertOnly input does not authorize full
upsert, changing pending pages or treating matching readback as universal completion proof.

<a id="small-conservative-recovery-boundary"></a>
When terminality cannot be established, keep the uncertain target unserved and its obligation visible. A bounded
replacement may use the same fixed input in another never-reused generation; never silently retarget the old
attempt or reuse its collection name. Bound replacement count/retained disk/cleanup backlog and stop writes on
exhaustion. The external server is not ours to kill merely to claim a fence.

## Stage 3A release gate

The complete-generation path is the correctness/reference implementation. Ship a corpus class only after its
predeclared #215 correctness/latency/throughput/resource budgets pass. Include current+building+reader-retained
collections, redb retained pages, maximum reader/window lifetime and cleanup backlog. Coalesce newer desired state
without unbounded work. Failed budgets require Stage 3B for that class, not arbitrary internal thresholds.

## Stage 3B optional incremental publication

Only after measured need: persist one fixed complete-source delta; reuse unambiguous unchanged occurrences; create
new points with InsertOnly; close replaced points once at a fixed exclusive validity boundary without creating
missing points; verify effects and scoring/optimizer convergence; atomically commit matching complete-source
metadata. Old views retain their original eligibility/evidence.

No physical point deletion. Profile changes, interval exhaustion or measured closed-point growth build a fresh
generation. Oversize delta uses fresh generation or explicit gap. Do not reinterpret an existing collection as the
incremental profile or use custom source shards as an unmeasured ranking/publication shortcut.

## Commit/install gap and restart

Durable current precedes installation. Startup verifies exact current collection/profile and complete evidence,
opens the matching snapshot and installs it before admitting queries. Missing/corrupt/conflicting bindings refuse
serving, not empty success. An empty valid generation still has explicit schema and complete inventory.

A restart resolves pending/cleanup and reconciles current inputs; importing a backup is a separate fresh-destination
operation. Copied pending records must not mutate the original installation's backend.

<a id="view-lifetime-and-whole-generation-cleanup"></a>
## View lifetime and whole-generation cleanup

One strong current Arc and weak retired views suffice. Queries clone current under a short lock; windows inherit
that guard. Last drop wakes cleanup and does no I/O.

RetiredCleanup names one exact whole collection. Delete only when it is not durable current, no pending/future
build can target it, no live view/window uses it and no unresolved targetable attempt remains. All guards matter,
not only the immediate predecessor. Reader drain is not writer quiescence.

The bridge performs terminal collection delete and exact logical-absence observation outside writer/view locks.
Never reuse the name. Unknown/partial/foreign state retains the obligation and bounded backpressure; one absence
observation does not settle an unresolved creator. Logical absence is not physical erasure. Remove obsolete
metadata only after its consumers are gone. Local object GC is separate stopped-root maintenance.

## Removal and verification

Delete old floor/journal/proof machines, caller-supplied readback, daemon publication ledger, possible-writer
platform, point reclaimer, raw pin registry and exclusive fixtures after callers move. Keep only concrete fixed
pending input, current/evidence/cleanup metadata and the small view manager.

After code run minimal locked Rust 1.98 check/Clippy for actual consumers. Focused connected cases cover initial/
empty/gap builds, lost responses, conflict, commit/install crash, restart, dropped notifications, edit/removal,
incomplete inventory, old guards, delayed effects versus cleanup, bounded recovery and declared Stage 3A budgets.
Stage 3B cases apply only if selected. No runtime success is claimed by this recipe.

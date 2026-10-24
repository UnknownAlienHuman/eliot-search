# Publication: real effects and one atomic metadata boundary

At c8e38b6, shared admission/machine/recovery code holds complete manifests and caller receipts without
redb/Qdrant effects. Daemon/control add competing machines. Keep useful validation/readback, not the machines.
R011 rechecks the replacement instructions at `7c7fd36c`; the corrections below are not implemented-state claims.

R028 at `37dbb198` traces daemon `publication_composition::Publisher`: it has no production caller;
`publication_fault_process.rs` includes it directly with a backend double. `propose` changes an in-memory
`PublicationFloor` and persists opaque intent bytes through `publication_codec::FileJournal`; it does not
commit a redb current view or call Qdrant. Its compensation trait and caller-supplied recovery booleans are
not real observations. `rebuild_composition` similarly creates plans/readback proofs without building or
switching a backend generation. The [daemon map](../../../bins/eliot-searchd/FUNCTIONS.md#query-publication-and-rebuild-boundaries)
names the cutover; do not connect these competing machines as intermediate production owners.

The [control map](../../search-control-redb/FUNCTIONS.md) owns evidence tables/snapshots and restartable build
input capture; [projection](../search-projection-planner/FUNCTIONS.md) owns changed-source input;
[preparation](../../search-prep/search-materializer/FUNCTIONS.md) owns wanted-input/result reconciliation;
[bridge](../search-qdrant-bridge/FUNCTIONS.md) owns SDK semantics and native-inference readback.
This file owns publication selection and effect order.

## Reconcile desired and published state

A scan of successful eligible preparations alone cannot discover a source removed from that set. If A is
published and then excluded/unregistered, no new prepared artifact for A is required or expected. Likewise,
a failed new input can disappear from a success-only scan while old points remain forever. These are gaps in
the former recipe, not reproduced runtime incidents.

Compare both current desired source state/outcomes and the current published source inventory. Use ordered
key pages/a merge over their source IDs under a consistent read snapshot; do not collect the whole corpus
into memory. A published-only source is work, not an absent queue entry. The existing published inventory
already remembers it, so no permanent deletion tombstone or completed-operation history is required.

| Desired state | Publication action |
|---|---|
| Usable current preparation, including supported zero units | Publish its exact source/prepared input and gaps; close replaced points. |
| Explicit terminal unsupported/failure outcome selected for publication | Publish a gap-only source outcome and close former points in the new view. Never label old evidence as the failed revision. |
| Confirmed source removal, unregistration or valid selection exclusion | Close its published points and remove its membership in the same view commit, even without new preparation. |
| Preparation still running, or incomplete/unavailable observation | Do not infer deletion or successful empty. Keep any prior published revision labelled as prior/stale and expose pending/gap state; it does not satisfy a current-input request. |
| Exact published input/outcome already matches | No new sequence or zero-effect pending operation. |

The [reconcile owner](../../search-source/search-source-reconcile/FUNCTIONS.md) distinguishes valid exclusion,
confirmed absence and incomplete observation. Publication consumes that distinction; it does not infer physical
absence from a filtered walk. A failed/slow source need not block unrelated complete-source updates. Do not
claim removal applied to new indexed views before its publication commits. Already pinned local work follows
[view/scope lifetime](../../search-query/search-access/FUNCTIONS.md); this is not a new access-revocation service.

Startup, queue overflow and bounded background reconciliation traverse both sides. Lost preparation,
publication or removal notifications must converge without another source edit. Repeated identical gaps are
also no-ops, not an infinite sequence generator. Actual wanted state and served published input remain distinct.

## Select and prepare

Under the serialized publisher choose expected current view and target sequence. Produce finite immutable
pages with actual Qdrant inputs, compact complete-source deltas and cleanup IDs. Validate row/byte ceilings
and cleanup capacity before external mutation. Commit one PendingPublication referencing the verified artifact.
Source selection, occurrence births, profile and readback targets are fixed; retry never rereads live paths.

Before pending/effects, obsolete preparation can be discarded/replanned. After effects may exist, do not
splice fresh bytes/births into its cursor. Complete fixed inputs on a settled route, or use isolated recovery
below; then reconcile newer inputs. Published inventory names actual revisions and gaps, not whichever heads
happen to be current when the operation finishes. Profile changes affecting retrieval use the generation path;
fail-only budget/test changes follow preparation's reuse rule, not an unconditional corpus rebuild.

No online local object deletion runs in this baseline. Stopped-root maintenance cannot race finalization and
its first durable reference.

## Published retrieval state is immutable

The redb snapshot preserves evidence rows, not Qdrant vectors/payload. Once published, a point's retrieval
input, vectors, profile and query-visible filter/group/scoring fields are not updated in place. A change
creates a different point or incompatible generation. Citation-only changes update evidence rows. Reusing
derived computation does not authorize mutation of an old point.

New points use valid_from=target. Closing writes one exclusive valid_until=target; repeat only that same
closure. Never reopen the interval or change valid_from. For published sequences below target the closure
leaves eligibility unchanged. A sequence label alone does not provide this guarantee.

Use the bridge's non-overwriting new-point operation. Narrow vector/payload APIs may build unexposed staging
data, not alter retrieval state used by a live view.

## Apply: observed state is not dispatch completion

An attempt has one absolute deadline/cancellation across RPCs. Restart/backoff may give a finite new attempt
budget; durable operation input is not an expired Instant. Nested calls cannot renew the budget. Cancellation
never erases possibly dispatched effects or permits infinite immediate retries.

Keep separate: what exact point/schema readback shows, and whether an earlier dispatched writer may still
execute against that route. A completed wait=true response settles that dispatch, not other attempts. If its
readback failed, retry the read rather than blindly resend the mutation. Preserve useful completion observations
in existing unfinished pending progress. A timeout, missing response, client drop or restart is not terminal
server evidence. Persist possible-dispatch state before effects; no completed-operation journal/bridge ledger.

Only after relevant dispatches settle can desired post-state mean done, expected pre-state/missing new item
mean retry, and foreign state mean stop. A mixed pre/post batch is not itself corruption. Native Document
inference follows the bridge's generated-vector contract, not fabricated client-known coefficients.

### Why readback-only retry was insufficient

```text
A: create P with open validity; response lost, A may still execute
B: repeat create; observe desired P; mistakenly declare all work settled
C: close P, publish a later view, eventually delete P
A: delayed create executes after C
```

A full upsert can reopen P even before deletion. Insert-only prevents overwrite while P exists, but can
reinsert it after deletion. Local serialization and strong ordering do not timestamp an old logical request
with its intended application order. A later successful read is not a fence. This is a design counterexample,
not a reproduced Qdrant failure.

### Small conservative recovery boundary

Do not send competing retries, advance visibility or retire affected IDs while an earlier writer may still
run. Continue bounded observation only where the selected backend can establish terminality. A sleep,
reconnect or matching readback cannot; do not assume an unavailable operation-status/cancellation API.

When terminality cannot be established, use the existing fresh-generation build path: retain the old route's
unresolved obligation, construct a never-reused generation from the fixed intended source/prepared inputs,
validate it and switch current atomically. This is explicit replacement, not changing the route underneath
the old cursor. Retain the old operation artifact until replacement durably takes over its required state,
and preserve unresolved old-route cleanup in that handoff.

The uncertain route receives no further publications. Existing readers may finish because late creates are
future-only for them and allowed closures do not change their eligibility; this relies on the immutability
rule. New requests use the replacement only after commit. Never recreate/reuse an abandoned collection name.

Bound replacement attempts, retained generations and cleanup backlog; on exhaustion stop new writes visibly.
This costs a rebuild and possibly retained garbage, not another recovery service. An external Qdrant endpoint
is not ours to terminate merely to claim a fence.

## Ordinary incremental commit

Decode the bounded metadata delta before opening the writer. After required terminal observations/readback:

```text
one redb transaction:
  compare expected current and exact pending
  apply complete-source evidence and published inventory changes
  current = target
  insert unfinished Qdrant cleanup, if any
  remove pending
commit
open read snapshot; verify current row is target
install snapshot-backed target under short manager lock
report visible
```

No object/RPC I/O or corpus-sized delta inside the writer. Do not begin another publication before snapshot
capture/install. Resolve uncertain commit from current/pending/cleanup. Failure after commit keeps old serving
state or refuses new serving until recovery; it does not report an uninstalled target visible. Old requests
read their old snapshot, never latest bindings under an old sequence label.

## Empty and unavailable

Nonempty -> supported empty closes former points and commits the new inventory. Initial empty needs no
upsert; last-source removal can publish a valid empty view. No sentinel point or stale nonempty success.
Gap-only publication follows the table above. Missing collection/metadata and incomplete initial build are
unavailable, not successful empty. Evidence for a failed revision is never borrowed from an older one.

## Initial build or incompatible profile

Use a new physical generation and separate source/evidence tables filled in bounded transactions. Capture
the exact intended input/outcome inventory according to [control's recipe](../../search-control-redb/FUNCTIONS.md#restartable-generation-input-capture)
before dispatching its Qdrant effects. One in-memory read transaction or a sequence number cannot restore
that selection after restart; a cursor reopened against changing current rows is not the original snapshot.

Staging stays unqueryable. Complete captured inputs and all required effects/evidence precede the pointer
switch; the final transaction does not copy the corpus. Resume only from the captured inventory and when
dispatch state permits, otherwise use isolated recovery. Source/config changes become follow-up work through
two-sided reconciliation. A zero-point generation still has selected schema and explicit complete inventory.
Generation names are installation-scoped and never recycled, including failed/abandoned targets.

## View and writer lifetime before cleanup

PublishedViewState owns a real redb snapshot plus route/profile/sequence. One current Arc and weak retired
refs suffice. Pin current only; windows inherit the guard. No manager lock across I/O. Last drop may wake
cleanup but does no deletion. Bound requests/windows and measure old-reader page growth/backpressure.

Transfer unfinished deletion to finite RetiredCleanup rows in the current commit. Points closed at N wait
for every relevant same-generation view below N; collections wait for all users. Neither may be reclaimed
while an unresolved writer could recreate the retired state. Reader drain alone is not writer quiescence.

For a settled route, exact absence completes deletion; partial deletion retries remaining expected IDs and
foreign identity stops work. Delayed deletion of never-reused retired IDs cannot delete newer points. For
an uncertain abandoned route, do not drop cleanup on one observed absence while a creator could still arrive.
Keep that unreachable data/obligation with bounded backlog and visible write backpressure until actual
terminality is established. Do not claim physical cleanup that the backend cannot prove complete.

Retire generation metadata after view/pending/cleanup consumers disappear. Local objects await stopped-root
GC; unresolved artifacts remain roots. Completed history is not retained.

## Restart is not restore

Ordinary restart opens durable current, checks pending possible-dispatch state before retry, reevaluates
cleanup and reconciles desired/published inputs. Old process-local readers are gone; a separate Qdrant
server may still execute old requests. Full-build input capture follows the control recipe, not a fresh scan
silently relabelled as old work.

A backup copy cannot enter this replay path. [Restore/import](../../search-runtime/search-retention/FUNCTIONS.md#explicit-restoreimport-into-a-fresh-destination)
uses committed source/prepared state in a fresh destination/generation. Copied pending/cleanup is archive
evidence, not authority over the original. Index loss requires a fresh verified rebuild, not empty success
or adoption of an unrelated collection.

## Remove and verify

Delete competing phase/receipt/observation/compensation machines, full-manifest encodings, raw pins and
separate reclaimer with their consumers. Preserve actual #598 private overlap, not its superseded programme.

After implementation check published-only removal with a lost notification; gap-only replacement and repeated
gap no-op; pending versus failed preparation; interrupted input capture and restart after capture; profile
change during rebuild; immutable old-view retrieval; partial settled batches; native readback; delayed create
across retry/close/delete; bounded isolated recovery; atomic commit/install gap; reader/writer cleanup;
restore isolation and old-reader growth. Minimal changed-owner compiler/Clippy first. No runtime success is
established by this document.

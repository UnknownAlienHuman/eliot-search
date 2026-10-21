# Publication: real effects and one atomic metadata boundary

At c8e38b6, shared admission/machine/recovery code holds complete manifests and caller receipts without
redb/Qdrant effects. Daemon/control add competing machines. Keep useful validation/readback, not the machines.

The [control map](../../search-control-redb/FUNCTIONS.md) owns evidence tables/snapshots;
[projection](../search-projection-planner/FUNCTIONS.md) owns changed-source input;
[preparation](../../search-prep/search-materializer/FUNCTIONS.md) owns wanted-input/result reconciliation;
[bridge](../search-qdrant-bridge/FUNCTIONS.md) owns SDK semantics and native-inference readback.
This file owns effect order. It is proposed implementation, not executed product evidence.

## Select and prepare

Under the serialized publisher, compare eligible current preparation outcomes with published source inputs.
An in-memory wake-up is only an optimization. Startup, queue overflow and bounded reconciliation must find
unpublished current inputs even if the process died after preparation commit but before notification.

Choose expected current view and target sequence. Produce finite immutable pages with actual Qdrant inputs,
compact complete-source deltas and cleanup IDs. Validate row/byte ceilings and cleanup capacity before any
external mutation. Commit one PendingPublication referencing the verified artifact. Its source selection,
occurrence births, profile and readback targets are fixed; ordinary retry never rereads live paths.

Before pending/effects, obsolete preparation can be discarded/replanned. After effects may exist, do not
splice fresh bytes/births into its cursor. Complete the fixed input on a settled route, or use the explicit
isolated recovery below; then reconcile newer inputs. Published inventory names actual revisions and gaps,
not whichever source heads happen to be current when the operation finishes.

No online local object deletion runs in this baseline. Stopped-root maintenance cannot race finalization
and its first durable reference.

## Published retrieval state is immutable

The redb snapshot preserves evidence rows, not Qdrant vectors/payload. Once published, a point's retrieval
input, vectors, profile and query-visible filter/group/scoring fields are not updated in place. A change
creates a different point (or a new incompatible generation). Citation-only changes update the evidence rows.
Preserve derived computation where reusable; that does not authorize mutation of an old point.

New points use valid_from=target. Closing a point writes one exclusive valid_until=target; repeat only that
same closure. Never reopen its interval or change its original valid_from. For every already published
sequence below target, this closure leaves eligibility unchanged. This and immutable retrieval state are
what let old queries continue; a sequence label alone is insufficient.

Use the bridge's non-overwriting new-point operation. Narrow vector/payload APIs are not a bypass for this
rule. They may build an unexposed staging target, not alter data used by a live published view.

## Apply: observed state is not dispatch completion

One execution attempt has one absolute deadline/cancellation across its RPCs. After restart/backoff a finite
new attempt budget is allowed; operation input is not an expired Instant. Nested calls cannot renew the
budget. Cancellation never erases possibly dispatched effects or permits infinite immediate retries.

Keep two questions separate:

1. What state does the exact point/schema readback currently show?
2. Can any earlier dispatched mutation still execute against that route?

A completed wait=true response settles that dispatch, not other attempts. If its readback failed, retry the
read rather than blindly resend the mutation. Preserve useful completion observations in existing unfinished
pending progress when needed. A missing response, transport timeout, client drop or restart is not a terminal
server observation. Persist enough possible-dispatch state before effects that restart cannot forget it;
there is no completed-operation journal or second bridge ledger.

Only after relevant dispatches are settled can desired post-state mean done, expected pre-state/missing new
item mean retry, and foreign identity/state mean stop. A mixed pre/post batch is not itself corruption.
Read back according to the actual input kind: native Document inference has the bridge's generated-vector
contract, not a fabricated expected sparse vector.

### Why readback-only retry was insufficient

```text
A: create P with open validity; response lost, A may still execute
B: repeat create; observe desired P; mistakenly declare all work settled
C: later close P, publish the next view, eventually delete P
A: delayed create executes after C
```

A full upsert can reopen P even before deletion. Insert-only prevents that overwrite while P exists, but
can still reinsert P after deletion. A local single publisher and strong ordering do not timestamp an old
logical request with its intended application order. One later successful read is not a barrier against A.
This is a design counterexample, not a reproduced Qdrant failure.

### Small conservative recovery boundary

Do not send competing retries, advance visibility, or retire the affected IDs while an earlier writer may
still run. Continue bounded observation only where the selected backend can actually establish terminality;
a sleep, elapsed client deadline, reconnect or repeated matching readback is not such evidence. Do not assume
an operation-status API or server cancellation acknowledgement that the selected client/server lacks.

When terminality cannot be established, use the existing new-generation build path, not more proof types:
record the old route's unresolved obligation, construct a fresh never-reused physical generation from the
fixed intended source/prepared inputs, validate it and switch current atomically. This is an explicit recovery
replacement, not changing the route beneath the old pending cursor. Keep the old operation artifact referenced
until the replacement has durably taken over its required state. Preserve unresolved old-route cleanup in the
same handoff; never drop it just because a replacement was built.

The uncertain route is not used for further publications. Existing old readers may finish because allowed
late creates are future-only for those views and allowed closures do not change their eligibility; this
argument requires the immutability rules above. New requests use the installed replacement only after commit.
Do not recreate/reuse an abandoned physical collection name. Late old writes cannot target the replacement.

A replacement attempt can fail too. Bound recovery attempts, retained generations and cleanup backlog. On
exhaustion stop new writes visibly rather than loop, manufacture success or delete evidence. This costs a
rebuild on unresolved dispatch; it is an explicit simplicity/availability trade-off. It uses the existing
pending/rebuild/cleanup owner, not a new recovery service. A configured external Qdrant endpoint is not ours
to terminate merely to claim a fence.

## Ordinary incremental commit

After required terminal observations and readbacks, decode the already bounded metadata delta before the writer:

```text
one redb transaction:
  compare expected current and exact pending
  apply complete-source evidence and published inventory changes
  current = target
  insert unfinished Qdrant cleanup, if any
  remove pending
commit
open read snapshot; verify its current row is target
install snapshot-backed target under short manager lock
report visible
```

No object/RPC I/O or unbounded corpus delta inside the writer. Do not begin another publication before
snapshot capture/install. On uncertain commit inspect current/pending/cleanup. Failure after durable commit
keeps old serving state or refuses new serving until recovery; it does not report uninstalled target visible.
An old request always reads its old redb snapshot, never latest bindings under an old sequence label.

## Empty, unsupported and no-work are different

Successful zero-unit preparation is publishable. Nonempty -> supported empty closes former points and commits
the new inventory; an initial empty source needs no upsert. Last-source removal can yield a valid empty view.
No sentinel point or stale nonempty success is needed to bypass EmptyUnitSet checks.

Unsupported/incomplete extraction stays an explicit source gap. Until a new outcome is published, old results
are visibly from the old published revision. A gap-only new outcome cannot attribute old evidence to the
failed input. Missing collection/metadata and unfinished initial build are unavailable, not empty success.
When exact published input/outcome already matches, skip redundant sequences and zero-effect pending work.

## Initial build or incompatible profile

Use a new physical generation and separate source/evidence tables, filled in bounded transactions. Staging is
not query-visible. Pending names fixed generation/inputs. Complete validation precedes atomic pointer switch
and cleanup transfer; the final transaction does not copy every corpus row.

Source changes during the build remain discoverable by comparing current input with built inventory.
Resume the same target only when its dispatch state permits it; otherwise use isolated recovery above.
A zero-point generation still requires selected schema and explicit complete inventory. Generation names are
installation-scoped and never recycled for a different build, including failed/abandoned targets.

## View and writer lifetime before cleanup

PublishedViewState owns a real redb snapshot plus route/profile/sequence. One current Arc and weak retired
refs suffice. Pin current only; windows inherit a guard. No manager lock across database/object/RPC I/O.
Last drop may wake cleanup but performs no deletion. Old readers retain pages; finite request/window lifetime
and write backpressure require measurement.

Transfer unfinished deletion to finite RetiredCleanup rows in the current commit. Points closed at N wait
for every relevant same-generation view below N; a collection waits for all users. In addition, no unresolved
writer may recreate/replace the state being retired. Reader drain alone is not writer quiescence.

For a settled route, exact absence completes a delete; partial deletion retries remaining expected IDs and
foreign identity stops work. Delayed deletes of never-reused retired IDs are harmless to newer points. For an
uncertain abandoned route, do not discard its cleanup obligation on one observed absence while a creator can
still arrive. Leave its unreachable data/obligation until actual terminality can be established, with bounded
backlog and visible write backpressure. Do not claim physical cleanup that the backend cannot prove complete.

Retire generation metadata only after view/pending/cleanup consumers disappear. Local object files await
stopped-root GC; unresolved-operation artifacts remain roots. Completed operation history is not retained.

## Restart is not restore

Ordinary restart opens durable current consistently, checks pending possible-dispatch state before retry,
and reevaluates cleanup. Old process-local readers are gone, but a separate Qdrant server may still run an
old request. Reconstruct missed preparation/publication work from current inputs/outcomes.

A backup copy cannot enter this replay path. The [restore recipe](../../search-runtime/search-retention/FUNCTIONS.md#explicit-restoreimport)
imports committed source/prepared state into a fresh destination/generation. Copied pending/cleanup is archive
evidence, not authority over the original. Index loss also requires a fresh verified rebuild, not empty success
or adoption of an unrelated collection.

## Remove and verify

Delete the old phase/receipt/observation/restore/compensation machines, full-manifest encodings, raw pins and
separate reclaimer after callers move. Preserve actual #598 private overlap, not its superseded programme.

After code check lost wake-up; stale preparation; empty/gap transitions; native readback; partial settled
batch; a delayed old create across retry/closure/deletion; no in-place vector/filter mutation of old views;
isolated recovery and repeated failure bounds; atomic metadata/current commit; commit/install gap; staging;
v1/v2/v3 cleanup with readers and writers; restart; old-reader disk growth and restore isolation. Minimal
changed-owner compiler/Clippy comes first. No runtime outcome is established by this document.

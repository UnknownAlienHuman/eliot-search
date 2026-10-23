# Query window — bounded retention, truthful progress

This file owns interactive page/window behavior. [Evidence resolution](../search-candidate-validator/FUNCTIONS.md)
owns ranges and failure meaning; the [protocol owner](../../search-provider-protocol/FUNCTIONS.md) owns framing
and cancellable I/O. Reviewed at `e5350809` on 2026-10-23. The replacement below is not implemented by this audit.

## Inspected code, not an assumed working window

- Daemon `continuation/kernel/catalog.rs` accepts an already materialized `StoreSearchResult`, clips its
  matches using a count allowance and retains the vector. `public_runtime_service/kernel/query.rs` calls
  `store.search` before that admission. `StoredMatch` owns five strings; a match count is not retained bytes.
  `Vec::truncate` drops removed elements but does not reduce the vector's allocated capacity. Existing DIRECT
  corpus limits do exist; this is an accounting/admission defect, not proof of unlimited production allocation.
- `expire()` runs on catalog access, counters and prepared delivery. The inspected line-session loop blocks
  awaiting another input and provides no independent expiry turn. A timestamp denies later use; it does not
  by itself release idle memory. The old catalog does not own a PublishedView guard, so a live redb leak is
  a risk of naively copying this lifecycle into the replacement, not an observed current failure.
- `catalog/prepared.rs` defers insertion/cursor change until successful output. Keep that useful property.
  The shared ContinuationStore and the daemon catalog are separate old implementations, not two required layers.

Move the small window into the real query/session owner and remove both old catalogs and separate handles.
No durable replan, token digest, source-catalog fence, issued-fingerprint archive or replacement window service.

## Admission and the finite result set

Before retrieval, choose the finite candidate allowance and reserve room for the active result and any requested
window in the existing query/session budget. Enforce response/allocation limits at the actual backend/read
boundary; clipping an already allocated vector is not admission. Account for vector capacity, owned strings,
optional cached bytes and temporary page/encoding copies. Count windows and retained bytes across the owner,
not independently per window with an unbounded session count. Use ordinary counters, not quota receipts.

Freeze one bounded ordered nomination set from that retrieval. Retain compact evidence references and rank,
not all source bodies or prebuilt cards. Continue slices this set; it does not rerun increasing-offset searches
or silently extend the candidate set. A new Search is an explicit new operation. Group only selected-page
reads by retained object; a cache must fit the same byte allowance.

Capacity pressure cannot silently turn unrelated results into a successful empty search. Reject a required
window before expensive work, or serve a bounded one-shot result with explicit unavailable continuation when
that response mode permits it. Do not advertise a locator and then discover that its state cannot be retained.
Unrelated windows are not silently evicted to manufacture capacity.

Window exhaustion means only that this retained nomination set has been handled. Report backend/window
truncation and candidate gaps separately. It proves neither corpus completeness nor absence of other matches.
A smaller backend nomination set or failed evidence cannot become a complete-negative result.

## Lifetime includes idle sessions

A window owns its pinned view, ordered references, cursor, issued-item indexes and one fixed absolute expiry.
It belongs only to the same live session. One-shot output returns no locators destroyed on child exit;
disconnect, failed session or restart drops session windows. Ordinary terminal output does not end a live session.

The existing session/event loop services the nearest expiry even without client traffic. Remove expired
records and cached bytes there, releasing their stored view references; a bounded scan of the finite window
set suffices. Do not add a cleanup service or rely solely on the next Continue/diagnostics request. Long source
reads and blocked output must not monopolize this control path.

Each Continue/Expand receives a new request deadline capped by the original window expiry. Actual I/O/work
already spent remains charged to the window's remaining allowance, including failed/cancelled attempts;
only unused reservations are released. Page retries do not reset TTL or lifetime work limits.

Expiry also cancels active work and forbids further emission, but removing the catalog entry cannot forcibly
drop a guard still owned by that work. The real I/O cancellation/termination boundary must release it. Until
then the reader is still live for publication/cleanup; do not report reclamation from the timestamp alone.
No global manager lock is held across retained-object or transport I/O.

## Page accounting and failure

1. Select the next finite prefix and reserve read/output work before evidence reads. Process in final rank order.
2. A successfully handled reference produces a card or a genuine candidate-local gap under the evidence recipe.
   Cancellation, deadline and aggregate request exhaustion are not missing-source gaps. Stop at the unhandled
   reference; do not advance past it or refill by scanning arbitrarily many later candidates.
3. If output space runs out, leave the next item for a later page. An intrinsically oversized item needs a
   bounded excerpt with truthful coordinates, a declared item-limit gap or an explicit error; it cannot cause
   endless empty pages. If the window's total allowance is exhausted, end it with that explicit reason rather
   than promising a retry that can never make progress.
4. Stage a capped response with the handled-prefix length and actual issued-card indexes. A permitted partial
   response accounts for its processed prefix and a separate stop reason, not fabricated gaps for its tail.
   Observed cancellation/session revocation emits no candidate page. No output means no cursor/issued change.
5. Resolve recoverable admission/validation before output. After a complete local frame write/flush, commit
   only that handled prefix and the locators actually carried by cards. The proposed slice length or card count
   is not necessarily the cursor delta. Partial/failed output commits nothing and closes the session.

Local flush is not peer acknowledgement. A lost session restarts Search; no exactly-once journal is required.
Serialize operations on the same session/window while the control reader still observes Cancel/EOF/expiry.

## Expansion and removal

Expand only a successfully issued item. Resolve its source/representation and finite neighborhood from the
same pinned evidence entry; the caller cannot select arbitrary offsets from another part of the revision.
Use the evidence owner's coordinate domain, not output-text length as a raw-source offset.

After the final page, retain only what issued-item expansion requires until fixed expiry; otherwise drop the
window immediately. References, caches and guards have ordinary ownership, not terminal cleanup receipts.

After code, run minimal changed-owner compiler/Clippy, then exercise: small versus spare-capacity result vectors,
full window capacity with a new query, multiple windows, idle expiry during indexing, active read at expiry,
partial output, cancellation before the next candidate, missing evidence versus request stop, oversized items,
window exhaustion versus corpus coverage, and mapped-text expansion. No runtime result is claimed here.

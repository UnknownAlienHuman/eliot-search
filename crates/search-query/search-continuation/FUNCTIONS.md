# Query window — one pinned view, one session, one cursor

This file owns paging and item-locator lifetime. [Evidence resolution](../search-candidate-validator/FUNCTIONS.md)
owns excerpt truth; [protocol](../../search-provider-protocol/FUNCTIONS.md) owns framing and transport failure.
This is the replacement recipe, not a claim that the current daemon implements it.

## Inspected current paths

At `44d7416c` the daemon has two separate session catalogues:

- continuation keeps a complete `Vec<StoredMatch>`, cursor, coverage, fixed expiry and a digest of the entire
  current source catalogue;
- result handles mint one random token and record per emitted match, with a separate TTL and expansion path.

Useful code exists: direct slicing, finite count ceilings and `PreparedPage`, which commits insertion/advance
only after successful local output. Keep those behaviors. Do not preserve the two catalogues or their proof-
shaped source fence merely because they already exist.

The inspected source also establishes these defects/limits:

- `cmd_search_page` constructs the full search result before continuation admission;
- count limits ignore owned strings, vector spare capacity and temporary encoded output;
- every page/expansion calls `list_sources`; `source_fence` serializes the whole current catalogue into a new
  buffer, so unrelated source changes invalidate every old token and page work is corpus-dependent;
- expansion rechecks the latest active source/revision instead of the pinned published view;
- each later page mints fresh per-item handles with a fresh TTL, which can outlive the original continuation;
- a handle record does not retain the emitted representation or match range and accepts arbitrary absolute
  offsets anywhere inside the raw revision.

These are static findings, not a reproduced production incident.

## One `QueryWindow`

One live session owns a small map of windows. A window contains only:

```text
window ID
PublishedViewGuard
final ordered EvidenceRef[]
next handled index
fixed expires_at
finite retained-item / metadata / optional excerpt-byte charges
issued item indexes needed for expansion
```

Each internal EvidenceRef points to an exact pinned evidence row and its representation/range; it is not a
copied source body. The public WindowItemRef is only `(window_id, item_index)` in that same session, resolving
one already emitted item. Do not store those public locators as the window's evidence. There is no separate
handle catalogue, item entropy, source-catalogue digest or per-page TTL.
Use the creating request's non-reused session-local ID as window_id. Expiry releases storage, not the ID;
an old locator must never resolve to a different later window in the same session.

Choose the fixed expiry while preparing the first terminal response, before output; validate and reserve the
new window's slot then. Continue/Expand get fresh request deadlines capped by that expiry and remaining lifetime
work limits. Access never renews it, and committing a frame never starts a new TTL. An exhausted
cursor may retain the window until expiry while emitted items remain expandable; otherwise release it promptly.

One-shot output exposes no live locator. Session end, EOF, failure, daemon restart or selected restricted-
session revocation drops its windows. A later invocation cannot resurrect them.

## Admission before expensive work

Reserve the window's nomination count and retained metadata/byte allowance before asking retrieval for more
candidates than can be retained. The backend may still perform corpus-dependent work; the reservation limits
what this request/window owns, not Qdrant's internal cost.

Account for:

- final retained item references and owned metadata;
- temporary candidate and evidence values before they are released;
- optional cached excerpts;
- the aggregate encoded terminal response.

Do not call a full current-source listing or hash the corpus to authorize a pinned view. The view already owns
its source/evidence population. A current source edit, rename or removal does not invalidate an old window whose
retained evidence still exists; new requests receive the newer published view.

If a required window cannot be admitted, return the selected bounded no-window response or a clear capacity
error. Never reinterpret capacity exhaustion as an empty or complete search.

## Page selection and cursor accounting

1. Starting at `next_index`, choose a finite candidate prefix that can plausibly fit the remaining item/read/
   output limits. Do not read the entire tail first.
2. Resolve evidence in final rank order. Each handled reference becomes a card or an explicit item-local gap.
   Cancellation, deadline or aggregate exhaustion stops before the next unhandled reference; it is not a gap.
3. A bounded aggregate gap may represent several actually handled references. Untouched candidates are never
   implied gaps. A single oversized item receives a defined bounded card/gap/error so the cursor cannot loop on
   an empty page forever.
4. Encode the complete terminal response into the protocol's capped local buffer. Stage the handled-prefix
   length and issued item indexes; card count or proposed slice length is not automatically the cursor delta.
5. After the entire terminal frame is written and locally flushed, atomically insert/advance this session's
   window state. Partial output commits neither insertion nor cursor and closes the session.

Local flush is not peer acknowledgement. On a lost connection the user repeats Search; no durable resume,
receipt or exactly-once protocol is required. Progress frames, if any, contain no cards, locators or cursor-
committing facts.

## Operation ownership

The protocol admits one active data request per session. Move the selected window into private operation-owned
pending state (or use the same exclusive ownership in the existing session task); leave no second acquisition
path. Do not clone its result vector or hold a mutable map borrow, window mutex or view-manager lock across
retained reads, encoding or transport I/O. The owned view guard may remain live across that work.

The control loop continues servicing Cancel, Close and expiry. It cancels the active operation directly rather
than waiting for a window lock. Idle entries may be removed; active state retains its view until work actually
stops. No global busy registry, revision-token protocol or separate window service is required.

Before any terminal byte, reserve the map slot and metadata, validate cursor arithmetic and stage the handled
prefix/emitted indexes. Enforce the deadline (including window expiry) while writing/flushing. After successful
complete flush, apply the prepared local update synchronously before the next dispatch: no allocation, new
source read, revalidation RPC or other fallible work after advertising the locator.

A healthy pre-output error/cancellation returns an existing unexpired window with cursor/emitted set unchanged.
Actual read/work charges remain spent; failure does not refund them. A failed new Search inserts no window.
Partial output/disconnect discards pending availability. Expiry/revocation cancels unfinished work and prevents
new acquisitions; after a completed terminal the same session owner applies the prepared update and then removes
expired availability without renewing it. Local flush is not peer acknowledgement.

The [protocol terminal rule](../../search-provider-protocol/FUNCTIONS.md#cancellation-versus-terminal-output)
decides which terminal wins. This is one session-owned update, not a second transaction or locking protocol.

## Expansion stays inside the emitted item

`Expand(window_id, item_index, before, after)` resolves the exact emitted item from the pinned view. `before`
and `after` are finite context requests in that item's declared representation/coordinate domain. The server:

- rejects an item index that was not successfully emitted;
- derives the permitted neighborhood from the exact retained representation and mapping records;
- clips on valid character/structure boundaries;
- maps the clipped region back to truthful source/page/region coordinates;
- never accepts arbitrary absolute offsets across the whole revision or switches to the latest source.

Raw bytes, normalized text and extracted document text are different domains. Expansion does not fabricate an
exact contiguous raw span for a non-contiguous or approximate mapping.

## Cutover and checks

Change query result retention, evidence resolution, protocol DTOs, daemon page/expand commands and CLI consumers
together. Move the useful prepared-output commit and direct slicing into the real session owner. Then delete:

- shared and daemon continuation catalogues;
- result-handle catalogue and per-item random tokens;
- full source-fence scans and latest-source revalidation;
- arbitrary handle-range expansion;
- durable replan/security/issued-fingerprint lifecycle machinery and exclusive fixtures.

Do not add a cross-catalog transaction, lease service or cleanup daemon to preserve a boundary being removed.

After implementation and minimal pinned compiler/Clippy, check admission before retrieval, retained/spare-byte
ceilings, unrelated-source change, old-view continuation, later-page TTL, final-page expansion, idle expiry,
active operation at expiry, missing evidence, cancellation, aggregate output limit, oversized item, partial
terminal output, one-shot absence of locators and mapped representation expansion. No runtime result is claimed
by this document.

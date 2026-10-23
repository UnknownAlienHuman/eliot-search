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
final ordered WindowItemRef[]
next handled index
fixed expires_at
finite retained-item / metadata / optional excerpt-byte charges
issued item indexes needed for expansion
```

`WindowItemRef` points to the exact pinned evidence row and its representation/range. It is not a copied
source body, public proof record or new handle token. Item locators are `(window_id, item_index)` on the same
session. There is no separate result-handle catalogue, item entropy, source-catalogue digest or per-page TTL.

The fixed expiry is chosen once when the first terminal response commits the window. Continue/Expand get fresh
request deadlines capped by that expiry and remaining lifetime work limits. Access never renews it. An exhausted
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

Serialize operations for the affected session/window, but hold no global view/window lock across Qdrant,
retained-object reads or transport writes. Idle expiry is serviced by the existing session/control event loop.
Removing an expired map entry prevents new acquisitions; an already-running operation retains its guard until
that operation actually ends.

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

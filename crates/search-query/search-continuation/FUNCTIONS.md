# Query window — source evidence and replacement behavior

This file owns page/window behavior. READMEs and issues link here, not to separate copied algorithms.
Framing and I/O are defined by the [protocol owner](../../search-provider-protocol/FUNCTIONS.md).

## Existing source at c8e38b6

The shared ContinuationStore is not the public daemon store. Its durable-replan/issued-fingerprint/security
model has no real view ownership. The daemon instead keeps a complete StoredMatch vector, cursor, expiry
and a whole-source-catalog fence, then allocates another result handle for each output item.

Keep direct slicing and prepared output commit. Remove both continuation catalogues and both handle stores
with their actual consumers. A result window is a field of the actual query/session owner, not a new service.

## Lifetime

A window retains one view guard, final ordered evidence references, cursor, fixed expiry and finite retained
item/byte limits. No durable query history, pin IDs, token signatures or repeated source-catalog scans.

Only the same live session can use its ID/item locators. Terminal completion does not close that session.
One-shot CLI mode does not return locators it is about to destroy. Session end, disconnect, failure or daemon
restart drops all its windows. A later independent invocation reports the locator unavailable; no resurrection.

Every Continue/Expand has a new request deadline and counters, capped by the unchanged window expiry and
remaining window budgets. A page must not reuse the completed Search deadline or extend TTL on access.

## Page selection and accounting

1. Select a finite prefix starting at next_index; reserve the relevant budgets before source reads.
2. Process in final rank order, reading only the candidate(s) needed for this response.
3. Each consumed reference becomes a card or an explicitly represented gap. Bounded aggregate gaps may stand
   for several consumed references; unprocessed candidates are not implied gaps.
4. Stop before the next reference whose complete representation cannot fit. It remains at the cursor for a
   later page. A single oversized item must produce a defined bounded gap/error or bounded excerpt according
   to the response policy, not an endlessly empty page.
5. Encode the response in a capped local buffer and stage its handled-prefix length plus successfully issued
   item indexes. The proposed slice length and number of cards are not necessarily that prefix length.
6. After full local frame write/flush, advance only by the handled prefix. Commit issued-item visibility only
   for actual cards in that frame. A first window becomes usable at that commit.
7. On partial/failed output, commit no cursor/insertion and close the session. Its windows are dropped.

This avoids both silent skipping when fewer items fit and infinite replay of known gaps. If a gap item is
consumed it must be accounted for in that response. Do not skip unseen candidates merely to fill a page.
Local flush success is not a peer acknowledgement; lost sessions restart Search rather than requiring an
exactly-once/receipt/replay protocol.

Serialize operations on the same session/window. Do not hold the global window or current-view manager mutex
across retained-object reads or a blocked transport write. Cancellation is serviced through the connection's
control path, not through that global mutex.

## Expansion

An item locator refers to a card successfully emitted within this window. Derive its exact revision and
permitted finite neighborhood from the pinned manifest; the caller cannot select unrelated arbitrary
revision offsets. Enforce actual selected scope and optional real restricted-session limits.

An exhausted cursor may leave the window alive until expiry for issued item expansion. With no issued
locators/cached content, release it immediately. Remove/drop releases references and view guard; no terminal
cleanup ledger is needed.

## Removal and focused checks

Delete shared durable-replan, token/security/issued-fingerprint machinery, legacy daemon catalogue/fences,
separate handle store and their exclusive fixtures. Move only useful source-range verification and prepared
output mechanics into the current owner.

After implementation/minimal Clippy: test a page smaller than the proposed slice, a missing-evidence gap,
one oversized item, failed first/next output, fresh per-page deadline versus fixed expiry, same-session
Continue, one-shot absence of locators, disconnect, cancellation and old-view release. No actual runtime
result is claimed in this map.

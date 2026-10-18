# search-exact — one authoritative scan

An exact request evaluates one explicit predicate over one explicit immutable source inventory. It reports
matches, what was actually scanned and why anything remained unread. Ranked Qdrant results never supply
this completeness claim.

## Current source

- [`literal.rs`](src/literal.rs), about 9 KiB, contains the useful Aho-Corasick single-literal matcher and
  has real daemon DIRECT callers.
- [`proof.rs`](src/proof.rs), about 64 KiB, builds caller-shaped plans, supplied inventory/readback records,
  reports, checkpoints and a second literal matcher. Its scan API has package-test callers and receives all
  source bytes in a preallocated `Vec<ExactItemReadback>`.
- [`spine.rs`](src/spine.rs), about 9 KiB, adds another pure denominator/completeness representation.

The proof/spine machinery does not enumerate the source owner, reopen revisions or hash returned bytes. It
therefore cannot turn matching fields into authoritative completeness.

## Target request

```text
ExactRequest
  resolved scope
  pinned PublishedView
  literal or selected regex predicate
  case/encoding semantics
  finite source/item/input/match/output limits
  one absolute deadline
```

The daemon derives an ordered immutable inventory from the pinned view. For each item it opens the exact
retained revision, verifies actual bytes/length/identity, scans one bounded source or chunk stream, emits
bounded `EvidenceCard` matches and releases the buffer before advancing.

Compile the predicate once per request. Keep matcher state across chunks of one source and reset it between
sources. Use the same Aho core for literal callers; remove `proof::find_literal` and its conflicting
non-ASCII semantics. Raw-byte and decoded-text modes remain explicit.

No client-visible compile/execute plan split, `PlanExecutionPermit`, denominator proof, checkpoint digest,
receipt chain or durable exact job is needed. Cancellation/restart returns a truthful incomplete result;
the caller may repeat the scan.

## Completeness

Terminal status is one of:

```text
complete_with_matches
complete_no_match
incomplete(matches + gaps)
invalid_request
```

Complete requires every item in the pinned inventory to be evaluated on the specified retained revision.
Missing bytes, unsupported encoding, source failure, byte/item/match limit, cancellation or deadline makes
the result incomplete. Matches remain useful but are never promoted to complete coverage.

The exact handler applies the already resolved local/restricted-client scope. It does not construct grants,
purge fences, handles or another source inventory. Optional paging/expansion uses the same `QueryWindow` as
ranked search.

## Optional predicates

Single literal is the baseline. Multi-literal is added only for an actual caller that benefits from one
combined scan; the Aho donor already supports it and no separate profile programme is required.

A selected regex uses one pinned `regex-automata` configuration with explicit syntax/Unicode/case and
compile/search ceilings. If safe cross-chunk semantics are not implemented, require one already bounded
contiguous source buffer; do not claim streaming or add a second fallback engine.

[FUNCTIONS.md](FUNCTIONS.md) maps the removal sequence. [AGENTS.md](AGENTS.md) governs implementation.

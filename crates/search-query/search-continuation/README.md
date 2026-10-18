# search-continuation

Bounded temporary state for paging one query result.

## Current source and consumers

[ContinuationStore](src/lib.rs) stores candidate-reference windows, issued fingerprint sets and
durable-replan descriptors in memory. `resume` returns a selection/effect description; it does not
call a planner, read source bytes or acquire/renew a pin. [Emission code](src/emission.rs) records
issued fingerprints after delivery. Terminal transitions already release candidate allocations;
`compact_terminal` removes eligible metadata.

The daemon's [native binding composition](../../../bins/eliot-searchd/src/access_composition/native_binding/process.rs)
constructs this store for lifecycle consumers. Public paging instead uses
[ContinuationCatalog](../../../bins/eliot-searchd/src/continuation/kernel/catalog.rs), alongside a
separate result-handle catalog. Shared store APIs alone do not connect indexed paging.

## One temporary owner

This is the target under
[query implementation](../../../docs/implementation/README.md#4-simplify-and-connect-the-query):

- Keep one bounded ordered window: resolved source/unit/revision references, selected scope and
  published view, actual pins needed to retain that view, next index and an absolute expiry.
- Reuse the daemon catalog's existing offset and
  [delivery commit](../../../bins/eliot-searchd/src/continuation/kernel/catalog/prepared.rs) mechanics
  in this owner. Move the useful implementation and connect its callers; remove the other engine.
  Do not retain shared and daemon continuation platforms side by side.
- Validate/read only the bounded page being prepared, produce compact cards and gaps, and advance the
  cursor after successful delivery. Keep rank stable. A partial frame ends the session; it does not
  become a successful page or an invitation to replay a partially emitted frame.
- Trusted-local expansion uses exact source/revision/range references. If a selected client requires
  opaque locators, they index items in this same owner. Finishing pagination must not discard backing
  items while issued locators remain valid; retain those items only until the original bounded expiry.
- Expiry, disconnect and cancellation release owned resources. Restart may expire the window.
  Bound both total retained bytes and item/window counts; no permanent query/issued-ID history.
- Use real published-view/source records and selected client restrictions. Do not rebuild and hash the
  whole source catalog on each page, silently change the corpus, or invent a valid pin/grant flag.

A query result is an ordered list of source/revision, location/range, excerpt and useful freshness/gaps,
plus an optional next-page reference. Execution, source validation and projection are direct steps
around this state, not four independent lifecycle systems. Durable replanning is absent from the normal
query path; an actual durable-job consumer would have to justify it.

[FUNCTIONS.md](FUNCTIONS.md) distinguishes current APIs, repeated work and useful existing mechanics.

# Published-view coherence, not snapshot proof

The old access snapshot protocol copied membership, policy, route and several security generations into
caller-shaped records, then compared digests/Booleans at multiple query phases. It did not read a current
owner and is superseded.

One successful query pins one immutable `PublishedView`. That view names the exact route/generation/profile,
selected membership inventory and evidence manifest roots used by the request. `resolve_scope` chooses a
non-widening subset from that same object. Retrieval, IDF population, exact scan and manifest lookup consume
that resolved scope; they do not accept an independent membership snapshot.

Coherence rules:

1. acquire only the current view through the publication owner;
2. resolve explicit/default source IDs against the pinned view; unknown/empty explicit selection never
   widens;
3. keep the view guard through backend, evidence reads, output and any live `QueryWindow`;
4. do not reacquire or compare a later view halfway through the request;
5. validate each Qdrant point against the pinned manifest and each excerpt against retained bytes;
6. release the guard on completion/cancellation/window expiry.

An optional restricted session contributes one additional allowed subset and cancellation owner. It cannot
broaden the view and does not travel as a grant digest/checkpoint transcript.

Delete access snapshot fingerprints, phase-currentness records, caller-supplied clean states and snapshot
integrity fixtures after consumers move. The actual immutable view plus retained evidence readback establish
the behavior the old protocol attempted to describe.

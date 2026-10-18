# Working on search-projection-planner

Read the root [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and local [function map](FUNCTIONS.md).
Change preparation, point identity, publication, bridge and query lookup together when their shared data
model changes.

The planner owns pure bounded transformation from one verified prepared view into:

- stable retrieval-point definitions and uploadable inputs;
- versioned evidence bindings for the new published view;
- exact create/retain/retire work;
- ordered manifest/change pages for publication.

Do not preserve current S11/S9.5 bytes, revision-bound Unit IDs or a whole in-memory manifest merely because
existing fixtures expect them. Source revision/range belongs in the view binding, not point identity or
mutable Qdrant citation payload.

Build pages once and pass their actual text/vector input to the bridge. Diff sorted point state separately
from view evidence; do not allocate two maps, clone all entries and hash another whole old/new preimage.
Planner code performs no redb/Qdrant effect and issues no receipt.

Verify a top-of-file insertion retains unaffected points, old/new views open their own exact ranges,
changed retrieval input creates a new point and batching preserves complete membership. Historical codec
bytes, package size or an old API boundary are not acceptance criteria.

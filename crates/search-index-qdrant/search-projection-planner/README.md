# search-projection-planner — bounded pages, not a whole-corpus proof

Turn one current prepared-unit stream into bounded Qdrant operation pages and versioned evidence bindings.
Do not build another source authority, complete in-memory manifest, publication journal or compatibility
contract.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [exact audit](FUNCTIONS.md),
[point identity](../search-point-identity/FUNCTIONS.md) and
[publication](../search-publication/FUNCTIONS.md).

## Verified current defect

Production source is 64,388 bytes, but its principal APIs are connected only to planner/publication tests.
`PublicationAdmissionOwners` has no production implementation.

The current plan receives actual vectors, hashes them, then drops them. It returns payload bytes and vector
digests which cannot upload either native BM25 text or the original vectors. It advertises 100,000 points /
64 MiB while the manifest codec caps one object at 4,096 entries / 8 MiB after points were already built.
It also binds point/scoring identity to revision, representation and current Unit ID, mixes evidence into the
point entry and repeatedly copies/encodes whole manifests.

## Target

```text
prepared units sorted by stable point identity
+ old point/view page iterator
→ bounded PointMutation[] with actual BM25 text/options or actual vectors
→ bounded ViewBinding[] with exact ObjectRef/revision/range
→ exact close/retain/create result
→ immutable operation pages
```

Scope/profile/route facts live once in the operation header. Stable point state and versioned evidence binding
are diffed separately. A source-coordinate shift may retain the point and write a new binding; changed
retrieval input/profile/incarnation creates a new point.

Use a two-way merge over sorted pages. Enforce point/encoded-byte limits before growth and poll one absolute
context between pages. Publication persists/apply/readbacks the pages; query resolves published bindings.
Planner code performs no redb/Qdrant effect and issues no receipt.

Delete complete plans/manifests, vector-digest substitution, generic canonical trees, full-map diff,
revision-bound scoring identity and fixture-only publication admission. If only a small page builder/merge
remains, merge it into preparation/publication rather than preserve the package name.

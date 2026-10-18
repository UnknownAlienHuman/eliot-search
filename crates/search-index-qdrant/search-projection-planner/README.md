# search-projection-planner

Turn one verified prepared view into bounded Qdrant work and a versioned published-view manifest.

Current planning is pure and checks complete UnitSet/vector coverage, but returns one whole plan. It drops
uploadable vector values, caps a whole manifest below advertised planner limits, reconstructs canonical
bytes repeatedly and mixes retrieval-point identity with current source evidence.

The target emits ordered pages containing:

- stable retrieval points and actual BM25/vector mutation input;
- `ViewManifestEntry` records mapping points to exact source revision/representation/range;
- exact create/retain/retire or whole-generation replacement work;
- expected Qdrant readback fields that the bridge can actually verify.

An unchanged retrieval point may be retained while the new view writes a new evidence binding. Changed
input/profile/occurrence incarnation creates a new point. Old pinned views keep their old manifest. Planner
code performs no durable or external effect.

Use a merge walk over sorted pages, not whole-corpus maps and repeated canonical trees. A new internal
format can replace current pre-release bytes with all readers/writers together.

[FUNCTIONS.md](FUNCTIONS.md) · [Point identity](../search-point-identity/S11_PROFILE.md) ·
[Implementation](../../../docs/implementation/README.md) · [Agent notes](AGENTS.md)

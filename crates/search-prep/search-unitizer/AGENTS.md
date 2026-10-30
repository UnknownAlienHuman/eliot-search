# Working on search-unitizer

<a id="implementation"></a>
Read [FUNCTIONS.md](FUNCTIONS.md) and only the changed materializer/projection boundary. Use one owned representation
and bounded descriptors; no second parser, object reader, manifest tree or self-referential prepared-object ID.

<a id="correct-results"></a>
Reuse useful range logic; preserve source/representation coordinates, UTF-8 boundaries, actual gaps and limits.
Unit position inside one immutable preparation is valid; it does not claim cross-revision continuity. Qdrant point
IDs belong to publication/projection. Old/new matching remains conditional Stage 3B work.

<a id="replacement-and-checks"></a>
Move the selected producer/consumer together, not every historical format. Remove duplicate replay, encodings and
fixtures with displaced APIs. After production code run minimal locked check/Clippy for actual consumers; later
range/mapping/memory cases stay in the recipe. Preserve user bytes and unpublished work.

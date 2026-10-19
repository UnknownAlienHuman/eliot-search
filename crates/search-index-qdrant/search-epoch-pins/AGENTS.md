# Working on search-epoch-pins

Read the root [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), local [source map](FUNCTIONS.md) and
[publication map](../search-publication/FUNCTIONS.md).

This package is a replacement/removal target. Implement one private current-view manager with a standard
mutex, one strong current `Arc`, weak retired references and lexical `ViewGuard` ownership. Do not repair
the raw pin registry.

Required behavior:

- `pin_current()` clones only the committed current view;
- publication holds the short manager lock across typed redb current-view commit and in-memory swap;
- a `QueryWindow` can clone only an existing live guard;
- no API accepts route, generation, sequence, owner ID, caller time or renewal;
- dead retired weak refs are pruned;
- last-drop only wakes cleanup; deletion executes elsewhere;
- restart reconstructs current and treats old ephemeral guards as gone.

Cleanup must evaluate all live views. Point IDs closed at sequence N wait until no same-generation live view
has sequence below N. Whole collection deletion waits until no live view uses that generation. The last guard
of the immediate predecessor is not sufficient when an earlier view remains live.

Delete route/epoch pin IDs, owner counts, snapshot/watermark/fence values, release-by-owner, continuation
renewal/binding history and P18 scale APIs with their callers. Move the small guard manager into publication/
query composition if no real package boundary remains.

After code, run minimal Rust 1.98 compiler/Clippy for publication/query/window/cleanup consumers and focused
commit-swap, v1/v2/v3, restart and last-guard behavior. Do not maintain old API-shape fixtures.

# Working on search-epoch-pins

Read the [implementation guide](../../../docs/implementation/README.md), local [source map](FUNCTIONS.md) and
[publication map](../search-publication/FUNCTIONS.md).

This package is a replacement/removal target. Implement one private standard-library current-view manager:
one strong current `Arc`, weak retired refs and lexical `ViewGuard` ownership. Do not repair the raw pin
registry.

Publication commits redb current/cleanup state first, then takes the short manager mutex to install the
target. Never hold the manager lock across redb fsync or object/Qdrant I/O. A query which pins old in that gap
remains valid and merely delays cleanup.

`pin_current()` clones only current. A window can clone only an existing live guard. No API accepts route,
generation, sequence, owner ID, caller time or renewal.

Cleanup evaluates every live view:

- point IDs closed at N wait until no same-generation live view has sequence below N;
- an old collection waits until no live view uses its generation.

The immediate predecessor's last guard is not enough when an earlier view remains live. Last drop only wakes
cleanup. Periodic scan is fallback. Restart reconstructs current from redb; old interactive guards disappear
while durable cleanup rows remain.

Delete pin IDs/owners/counts, raw route/epoch acquisition, release-by-owner, renewal, snapshots/watermarks,
continuation bindings, P18 APIs and shape fixtures. Move the small manager into publication/query composition
if no real package boundary remains.

After code, run minimal Rust 1.98 compiler/Clippy for publication/query/window/cleanup consumers and focused
commit→install gap, v1/v2/v3, restart and dead-weak behavior.

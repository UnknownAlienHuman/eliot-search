# Historical control cutover marker

The `control-cutover.v1` marker and its migration workflow are not part of the current-format product.
Unsupported pre-release roots fail before writes and remain untouched; baseline startup does not stage,
import, quarantine and atomically reroute an old control authority.

One useful implementation detail survives as an internal donor: the old cutover artifact publisher uses a
create-new temporary file, file sync, no-clobber hard link, temporary removal, directory sync and final
readback. Extract that small immutable-finalization mechanism into the shared object store, with a unique
concurrent-safe stage and an honest Windows durability profile.

Do not preserve the marker codec, replay classification, migration status, old serve-path compatibility or
its tests merely to keep that donor code. See the
[object-store map](../search-source/search-revision-store/FUNCTIONS.md) and [FUNCTIONS.md](FUNCTIONS.md).

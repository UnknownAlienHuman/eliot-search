# Retained source and prepared objects

One plain immutable reader/writer serves ingestion, preparation and evidence. Source membership and query scope
belong elsewhere. [FUNCTIONS.md](FUNCTIONS.md) is the only object algorithm.

## Plain storage first

[Local scope](../../../docs/architecture/LOCAL_SCOPE.md) selects plain storage only, with no storage-mode selector,
key dependency or future encrypted integration. Preserve unpublished work and old user data; neither authorizes
another writer or delays this implementation.

<a id="one-object-algorithm"></a>
## Publication

Follow [immutable publication](FUNCTIONS.md#immutable-publication).
<a id="windows-durability-is-an-explicit-profile-decision"></a>
The [root helper](../../search-runtime/search-runtime-owner/FUNCTIONS.md#shared-no-clobber-publication) owns private
same-parent staging, no-clobber persistence and honest durability limits. Do not copy a platform algorithm here.

## Reads

Follow [read rules](FUNCTIONS.md#reads): exact retained reference, actual integrity and bounds, not live-path fallback.

## Removal

Move real consumers before deleting duplicate writers/legacy wrappers. Writer-owned stage cleanup stays with
object I/O; final-object reachability belongs to [stopped-root maintenance](../../search-runtime/search-retention/FUNCTIONS.md#stopped-root-local-object-collection).
No key deletion, automatic decrypt/migration or user-original cleanup.

<a id="current-problem"></a>
## Source inspection

[Prior inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/6ae24197a1be8695ede5bb7d12dfaf97f5bca1fc/crates/search-source/search-revision-store/README.md#source-inspection)
retains historical source observations. Recheck actual callers; these instructions are not implemented storage.

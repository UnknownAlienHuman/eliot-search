# Retained source and prepared objects

One immutable object reader/writer serves source ingestion, preparation and evidence. It does not own source
membership or query scope. [FUNCTIONS.md](FUNCTIONS.md) owns the implementation; this page only routes it.

## Plain storage first

Implement the plain path now. Optional #598 encryption joins the same ObjectRef/reader/writer boundary only
after its accepted public handoff. Preserve actual unpublished overlap; it is not a prerequisite for unrelated
plain-storage work. Do not create a competing encrypted writer or a second source/query path.

<a id="one-object-algorithm"></a>
## Publication

Use the [immutable publication recipe](FUNCTIONS.md#immutable-publication), not a second algorithm here.

<a id="windows-durability-is-an-explicit-profile-decision"></a>
The [root platform owner](../../search-runtime/search-runtime-owner/FUNCTIONS.md#windows-finalization) owns the
shared finalizer, supported-path checks and durability limits. An internal legacy helper is inspection material,
not permission to substitute another Windows move/rename recipe.

## Reads

Use the [object read rules](FUNCTIONS.md#reads). Exact retained references and actual integrity, not the live
source path or echoed metadata, supply evidence.

## Removal

Writer-owned stage cleanup follows the object recipe. Final-object reachability and collection belong only to
[stopped-root maintenance](../../search-runtime/search-retention/FUNCTIONS.md#stopped-root-local-object-collection).
Move real callers before removing duplicate writers or legacy formats; preserve user originals and sole copies.

<a id="current-problem"></a>
## Source inspection

The [earlier kernel inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/4cb8dc4d61924ee0a1a2f3ce056b7086ac45c332/crates/search-source/search-revision-store/README.md#current-problem)
records the legacy/duplicated paths. Recheck current callers at implementation; target instructions do not mean
the persistent product or platform behavior has been executed.

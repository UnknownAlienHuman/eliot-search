# Source history — forwarding layer to remove with callers

This legacy package forwards revision registration, namespace cutover and registry restoration over
`search-source-records`. The replacement uses the existing object/source/control owners, not another history service.

## Replacement

The [source-row recipe](../search-source-registry/FUNCTIONS.md#current-state-and-conditional-application) owns
current references and guarded updates. The [object recipe](../search-revision-store/FUNCTIONS.md) owns retained
bytes. Keep the useful values with those consumers; do not move old operation replay or compatibility codecs.

Plain current-state work does not wait for optional #598 encryption. Protect actual unpublished object/key
changes and coordinate only that overlap. A future encrypted profile does not gate unrelated type relocation.

<a id="delete-after-270271-cutover"></a>
## Delete after current consumers move

Trace actual current callers; move required value fields/validation, then delete displaced restore/history/
namespace wrappers, reexports and exclusive fixtures with their dependencies. Do not infer that main already
contains the removals from an unmerged branch. Unsupported roots and sole retained copies remain intact.

[Stopped-root maintenance](../../search-runtime/search-retention/FUNCTIONS.md#stopped-root-local-object-collection)
owns complete reachability and final-object deletion. This forwarding package has no separate GC or retention policy.

<a id="verified-source-inventory"></a>
## Historical inventory

[Earlier source inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/4cb8dc4d61924ee0a1a2f3ce056b7086ac45c332/crates/search-source/search-source-history/README.md#verified-source-inventory)
retains the old file sizes and source families. They are not a fresh recount or a required package topology.

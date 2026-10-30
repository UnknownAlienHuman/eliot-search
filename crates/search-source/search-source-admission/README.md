# search-source-admission

Small selection/exclusion checks around one stable read. No approval service, policy database or provenance
archive. [FUNCTIONS.md](FUNCTIONS.md) owns the checks, failure semantics and removal boundary.

<a id="small-current-policy"></a>
<a id="current-state"></a>
## Work on one connected path

Start with the explicit-file consumer. Keep existing exclusions, actual byte limits and refusal reasons;
pass the acquired body to the shared writer. Source/preparation/storage owners perform their own work without
copying it into an admission record. A legacy review requirement remains a refusal, not a feature to implement.

<a id="remove-after-direct-caller-migration"></a>
<a id="completion"></a>
Move real callers and remove displaced receipt/batch/restore APIs with their fixtures. The
[source map](../README.md#ownership) routes adjacent boundaries. No whole-policy rewrite is needed first.

<a id="verified-current-source"></a>
## Historical evidence

The [earlier inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/2aa65104d903200806decec83c4713389608ba15/crates/search-source/search-source-admission/README.md#verified-current-source)
contains the old package-size and caller findings. They are not a current measurement or deletion permission;
recheck actual imports, feature gates and consumers at the code cut.

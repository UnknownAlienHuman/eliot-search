<a id="qdrant-process--source-audit-and-replacement-map"></a>
# Qdrant supervisor — remove the unused process platform

The selected product uses the [bridge's local connection](../search-qdrant-bridge/FUNCTIONS.md#local-connection).
The operator starts/stops Qdrant. ELIOT owns its requests and collections, not that external process.

<a id="target-modes"></a>
<a id="externalconfigured-endpoint--baseline"></a>
## Baseline cutover

Trace daemon construction and the bridge's actual connect callers. Supply endpoint, an existing key only if
needed, and execution limits directly. No executable hash, supervisor binding, credential lease, containment
receipt or process ID is a prerequisite for connecting, building a generation or searching it.

If startup fails, report the actual dependency error and clean only ELIOT-owned work. Do not adopt/kill a listener,
rewrite server configuration, auto-download/upgrade the server or retry through another transport/backend.

<a id="code-to-keep-or-move"></a>
<a id="delete-after-cutover"></a>
## Removal boundary

Search current Rust callers, module declarations, Cargo features, test targets and scripts. Move any genuinely
used endpoint/config helper to its consumer; do not preserve a forwarding crate. Remove the package only when
its callers and dependency edges are gone. Delete exclusive launch/receipt/oracle fixtures with that code.
Keep actual bridge schema, authentication, timeout and readback behavior. No user file or server data is deleted.

<a id="managed-child--optional-convenience"></a>
<a id="managed-launch-sequence"></a>
<a id="actual-windows-containment"></a>
## Deferred managed-server convenience

No managed-server implementation or config variant is selected for the baseline. A later explicit request must
first demonstrate a consumer and decide who owns the real child; it must not delay the current path. Do not
prebuild a Job/ACL wrapper, platform strategy catalogue, restart state machine or generic supervisor now.

<a id="verified-public-main-source"></a>
## Source evidence

The [earlier audit](https://github.com/UnknownAlienHuman/eliot-search/blob/7ed46f79f07a23f01f75f5e85a96248dc42e280b/crates/search-index-qdrant/search-qdrant-supervisor/FUNCTIONS.md#verified-public-main-source)
records test-only construction, caller-made containment claims, fixed files, manual HTTP probing and incomplete
child cleanup at its inspected revision. Recheck actual code before cutting it; this page claims no new runtime audit.

<a id="focused-verification"></a>
## Checks

After source changes, check/Clippy the real bridge and affected daemon/feature consumers. Verify statically that
normal connect has no supervisor/secret-lease constructor. Later connected connection failures, first build and
restart cases live in the bridge and stage checklist. Do not build a managed-process test platform first.

# Root owner

The target is one current-format root and one process-lifetime exclusive guard. Inspection is read-only;
initialization is explicit and publishes the root manifest last. A recognized empty incomplete initialization
may be explicitly discarded and retried. There is no baseline resume, replay journal or migration-on-open.

[FUNCTIONS.md](FUNCTIONS.md) owns classification, initialization, platform finalization, discard and shutdown.
[AGENTS.md](AGENTS.md) contains local editing constraints. Read the
[Foundation stage](../../../docs/implementation/README.md#1-foundation--invocation-settings-and-root) for scope.

The existing lease/epoch/receipt and command-custody machinery is not the target design. Trace current callers,
retain useful native checks/handles and remove displaced routes with consumers. Historical source inventory is in
[the pre-R055 recipe set](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/crates/search-runtime/search-runtime-owner/README.md),
not a current size claim or obligation to preserve packages.

Status and executed evidence belong to the [registry](../../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

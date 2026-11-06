# eliot-searchd

The binary currently provides four plain-root operations: --inspect-data-root, --initialize-data-root,
--check-existing-root-owner and --discard-incomplete-data-root. Native arguments resolve to typed root state
before the selected search-runtime-owner operation. Control-format operations use search-control-redb.

Source ingestion, preparation, Qdrant publication and framed search are not connected to this binary.
The removed DIRECT, migration, sealed, scan and loopback routes have no active replacement command here.
The package defines the installed binary only; there is no feature or library target to enable those routes.

[FUNCTIONS.md](FUNCTIONS.md) describes the actual command path and later composition;
[AGENTS.md](AGENTS.md) gives editing constraints. Follow the
[delivery sequence](../../docs/implementation/README.md#delivery-sequence) for the next connected change.

Final root/control integrity and child-pipe fixes require checks at the final source revision.
The [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) records executed scope;
earlier checks and helper probes are not final-head or installed-baseline qualification.

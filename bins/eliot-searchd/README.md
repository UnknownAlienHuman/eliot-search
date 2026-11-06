# eliot-searchd

The binary currently provides four plain-root operations: --inspect-data-root, --initialize-data-root,
--check-existing-root-owner and --discard-incomplete-data-root. Native arguments resolve to typed root state
before the selected search-runtime-owner operation. Control-format operations use search-control-redb.

The additional --retain-file --source-file PATH command connects one bounded native read to verified immutable
object storage and a guarded current source row in the same control/control.redb. Preparation, Qdrant publication
and framed search remain unconnected.
The removed DIRECT, migration, sealed, scan and loopback routes have no active replacement command here.
The package defines the installed binary only; there is no feature or library target to enable those routes.

[FUNCTIONS.md](FUNCTIONS.md) describes the actual command path and later composition;
[AGENTS.md](AGENTS.md) gives editing constraints. Follow the
[delivery sequence](../../docs/implementation/README.md#delivery-sequence) for the next connected change.

Root and S1 native evidence have separate exact-source scopes; S1 final gates/builds/basic execution passed at 06bb6877.
The [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) records executed scope;
earlier checks and helper probes are not final-head or installed-baseline qualification.

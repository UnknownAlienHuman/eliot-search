# eliot-searchd

The daemon provides root operations, --index [--index-path PATH], retain-file, retained-only prepare-source,
state-only publish-prepared and --search-stdio. Index requires an initialized current data root and composes
scan/retain/prepare/membership/publication under one root guard and original deadline. Explicit PATH admits and
refreshes only that scope; bare index reloads durable File/Tree admissions and scans their deduplicated union.
Explicit File wins over Tree ignores; only confirmed absence removes current membership, not ignored presence.
Search uses the separate serviced framed child and returns verified retained units/citations without writes.

Only the bridge uses Qdrant SDK types; the operator starts the same-host service. Unknown old build effects
can be preserved in retired obligations while a fresh successor succeeds; cleanup never deletes an unresolved
target or treats readback as settlement. No manual reset of the original failed root is needed.

[FUNCTIONS.md](FUNCTIONS.md) owns the operation paths; [AGENTS.md](AGENTS.md) gives constraints.
[Registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) separates selection-reload and earlier native
scopes. Admission/reload are durable; R1 remains partial because no foreground periodic trigger/lifetime/interval
is connected. Legacy source rows do not become admissions or bypass matching-preparation validation.

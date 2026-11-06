# eliot-search

The CLI provides local help/version, four one-shot root operations, retain-file --file PATH and
prepare-source --source-id UUID [--unit N], and publish-prepared --qdrant-url URL [--query TEXT] [--limit N].
The same owned daemon child retains/prepares a file or publishes the complete prepared corpus to Qdrant and
returns real point/evidence metadata. Read-only retained-excerpt search and live queries remain U1/later work.

Read [FUNCTIONS.md](FUNCTIONS.md) for exact syntax, daemon selection, response handling and future client work;
[AGENTS.md](AGENTS.md) gives editing constraints. The
[current dispatch map](../../docs/implementation/CODE_MAP.md#existing-dispatch-to-replace) names the actual files.

Root, S1, P1 and Q1 qualification have distinct source/binary scopes in the
[registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status). Stored-unit readback does not complete
the retained-source → Qdrant → verified-excerpt product or qualify an installed baseline.

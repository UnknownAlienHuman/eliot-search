# eliot-search

The CLI provides local help/version and four one-shot root operations: inspect-data-root, initialize-data-root,
check-existing-root-owner and discard-incomplete-data-root. It launches an owned eliot-searchd child for the
selected operation. The CLI has no source-ingestion, search, Qdrant or live-query command.

Read [FUNCTIONS.md](FUNCTIONS.md) for exact syntax, daemon selection, response handling and future client work;
[AGENTS.md](AGENTS.md) gives editing constraints. The
[current dispatch map](../../docs/implementation/CODE_MAP.md#existing-dispatch-to-replace) names the actual files.

The root and retained-file commands are not the connected retained-source → Qdrant → verified-excerpt product.
Final root/control integrity and child-pipe fixes still require qualification at the final source revision.
Executed evidence belongs in the [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status);
source inspection, helper probes and earlier binary runs do not qualify later changes or an installed baseline.

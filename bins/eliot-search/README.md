# eliot-search

The CLI provides local help/version, four one-shot root operations and retain-file --file PATH. The retained-file
operation acquires one bounded file, verifies its stored ObjectRef and commits a guarded current source row
through the same owned eliot-searchd child. Preparation, Qdrant, search and live queries remain unconnected.

Read [FUNCTIONS.md](FUNCTIONS.md) for exact syntax, daemon selection, response handling and future client work;
[AGENTS.md](AGENTS.md) gives editing constraints. The
[current dispatch map](../../docs/implementation/CODE_MAP.md#existing-dispatch-to-replace) names the actual files.

The root and retained-file commands are not the connected retained-source → Qdrant → verified-excerpt product.
Root qualification and the S1 binary/library native batches have distinct exact-source scopes.
S1 Windows debug gates/builds and final basic execution passed at source 06bb6877. Exact native scopes belong in the [registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status);
source inspection, helper probes and earlier binary runs do not qualify later changes or an installed baseline.

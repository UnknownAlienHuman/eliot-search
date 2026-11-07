# eliot-search

Use an operator-started same-host Qdrant and initialize a new application data root once, then:

    eliot-search index PATH --qdrant-url URL --data-root ROOT
    eliot-search search --query TEXT [--limit N] --qdrant-url URL --data-root ROOT

Index refreshes an explicit file or ignore-aware tree through the same owned daemon child; it does not
initialize the root or start Qdrant. Search returns verified full retained units/citations and is read-only.
Root, retain-file, prepare-source and state-only publish-prepared remain available for direct operations.

[FUNCTIONS.md](FUNCTIONS.md) owns syntax, failure/output and child lifetime; [Quickstart](../../QUICKSTART.md)
gives the normal path. [Registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) keeps exact source/native
scopes. R1 is partial: durable admission, writable-startup reload and actual periodic triggering remain open.

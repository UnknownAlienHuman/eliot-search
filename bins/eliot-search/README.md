# eliot-search

Use an operator-started same-host Qdrant and initialize a new application data root once, then:

    eliot-search index [PATH] --qdrant-url URL --data-root ROOT
    eliot-search search --query TEXT [--limit N] --qdrant-url URL --data-root ROOT

With PATH, index admits and refreshes only that file/tree. Without PATH, a new invocation reloads all durable
selections and refreshes their deduplicated union. No admission means INDEX_SELECTION_REQUIRED, even with old
retain-file rows. Index does not initialize the root or start Qdrant. Search returns verified full retained
units/citations and is read-only.
After admission, watch [--interval-ms N] repeats bare Index in the foreground. It takes no PATH, defaults to a
30-second after-completion interval, and cleans each owned child before cancellable sleep. Ctrl-C stops it.
Root, retain-file, prepare-source and state-only publish-prepared remain available for direct operations.

[FUNCTIONS.md](FUNCTIONS.md) owns syntax, failure/output and child lifetime; [Quickstart](../../QUICKSTART.md)
gives the normal path. [Registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) keeps exact source/native
scopes. Admission/reload/watch are connected; R1 remains partial until remaining Stage 3A qualification,
then Exact/windows, maintenance and release qualification.

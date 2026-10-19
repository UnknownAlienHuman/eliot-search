# Current-state diagnostics

Diagnostics are bounded read-only observations of the current typed application state and its actual
external owners. They are not receipts, recovery authority, grants or proof that the product is ready.

## Useful observations

A diagnostic/status command may report:

- schema / installation / selected root identity;
- counts and bounded summaries of current roots, sources and source heads;
- current preparation references and gaps;
- `CurrentPublishedView` route, generation, profile and manifest reference;
- presence and cursor/state of the single `PendingPublication`;
- actual Qdrant connection/collection/readback status from the Qdrant adapter;
- immutable-object verification failures and bounded orphan/cleanup backlog;
- explicit unsupported-schema, unreadable-store or excluded-root state.

Read one coherent redb transaction. Point-read exact records or iterate only a bounded current table needed
for the requested status. Do not read source bodies, vectors, every manifest entry or historical operations
merely to display health.

## Not readiness

`metadata readable`, `redb opened`, `Qdrant reachable` and `no pending publication` are separate facts.
None alone proves that a source is retained, the index matches the visible view or a query can return
verified evidence. Product readiness comes from the connected source→object→preparation→publication→query
path and its direct readbacks.

Do not infer authority from a process-local pointer or old `Arc`. The committed typed row is durable state;
the publication owner controls the live pointer and view guards. Diagnostics observe these owners but do
not mutate, suspend, repair or acknowledge them.

## Remove with the old journal

Delete `JournalWriteCounters`, acknowledged-call counters, receipt cardinality/byte limits, publisher
alignment ceremonies, generic pending-mutation health, durable quarantine-marker inspection and
operation-ledger consistency status after their old APIs lose callers. They diagnose the superseded journal
platform rather than the product.

A failed or cancelled diagnostic performs no repair and returns no partial `healthy` verdict. Recovery is
an explicit owner operation using the exact affected typed state or pending publication artifact.

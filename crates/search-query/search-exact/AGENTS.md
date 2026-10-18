# Working on exact scans

Read the [architecture](../../../docs/architecture/README.md),
[query cutover](../README.md#direct-operations-not-a-recipe-operating-system), [README](README.md) and
[function map](FUNCTIONS.md).

Implement one server-owned `ExactRequest` over the source inventory of a pinned `PublishedView`. Read and
verify each retained revision as work advances; do not preload the corpus or accept caller inventory,
readback, completeness, permit or receipt records.

Keep the existing Aho literal matcher, compile once, preserve state across chunks of one source and reset
between sources. Remove the second `proof::find_literal` matcher and unify ASCII-insensitive semantics.
Optional regex uses one explicit `regex-automata` configuration only when requested by a real caller.

Return source-backed evidence cards plus scanned-item/byte accounting and truthful gaps. Complete negative
requires every inventory item. Cancellation, limit, missing revision or unsupported input is incomplete,
not success.

Remove public compile/execute scan plans, checkpoints, proof digests, durable-job lifecycle and handle
minting with their consumers. Use the shared `QueryWindow` only for paging/expansion. Do not create another
inventory/access/capability/qualification framework.

Verify the connected daemon request: frozen inventory, exact revision bytes, cross-chunk overlaps, source
boundaries, non-ASCII behavior, byte limits before reads, cancellation and complete/incomplete status.

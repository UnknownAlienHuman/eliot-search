# Working on search-handles

Read the root [architecture](../../../docs/architecture/README.md), current
[implementation](../../../docs/implementation/README.md) and the
[query cutover](../README.md#one-query-window).

This package is a removal target, not the future owner of result lifetime. Trace every real
`SearchSourceHandle` producer/consumer, change the result contract with those callers and move only the
useful exact-range/readback mechanics into the one query window or retained-source reader.

Do not add:

- another token format, digest domain or collision registry;
- durable handle persistence, leases, tombstones or cleanup journals;
- synthetic grants/owner epochs to satisfy obsolete constructors;
- compatibility wrappers that keep the old store reachable.

Keep real byte/range/deadline limits and selected restricted-client checks. For the baseline same-user
local client, a result locator is not an authorization capability. Verify the connected query, expansion
and terminal cleanup; package-local fixture preservation is not a completion criterion.

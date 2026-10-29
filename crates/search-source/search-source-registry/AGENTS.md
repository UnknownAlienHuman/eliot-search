# Working on search-source-registry

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [README](README.md) and
[source map](FUNCTIONS.md). Trace the native source command, retained-object writer, redb transaction and
publication consumer before preserving a registry API.

Own current source metadata only: selected roots, source identity/location, current retained revision and
availability. Source bytes belong to the revision store. Immutable query meaning belongs to
`PublishedView` and its manifest.

Change package types and their real daemon/control consumers together. Do not create synthetic receipts,
control ports or compatibility wrappers to satisfy old constructors. If membership, portfolio, cutover,
history, restore or currentness code has no product caller, remove it with its fixtures rather than
building another consumer.

Keep source-head transactions short and typed. The immutable object exists and is read back before the
head changes. Unknown commit is resolved from actual control state. A path scan, matching digest field or
in-memory `SourceRegistry` is not durable source truth.

`search-source-records` and `search-source-history` are relocation packages, not protected architecture.
Merge useful current records into their actual owner and delete forwarding boundaries when callers move.
Unsupported old roots stay intact; no universal migration must land first.

Preserve #598's active private work and consume its accepted retained-source result without assigning a
competing writer. Verify the connected acquire → object → current head → restart behavior; package tests
and schema goldens alone are insufficient.

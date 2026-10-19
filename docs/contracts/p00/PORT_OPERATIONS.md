# Historical P00 port operations

The previous P00 contract froze one shared trait and generic operation semantics for every planned subsystem.
It produced `search-ports`: 23 public traits, 80 registered methods, a generic relative-deadline context,
mutation identity/receipt records and a conformance fake catalogue.

That contract is retired. See [`search-ports/README.md`](../../../crates/search-ports/README.md), the
[current dependency graph](../../handoff/DEPENDENCY_GRAPH.md) and the
[implementation guide](../../implementation/README.md).

## Replacement

Use direct current owners:

| Need | Current boundary |
|---|---|
| current settings/source/view/publication state | typed redb methods |
| immutable source/prepared/publication artifacts | one object writer/reader |
| source inventory/read | source owner methods |
| preparation | direct materializer/unitizer/projection functions |
| Qdrant | private bridge operations with exact vendor semantics |
| view lifetime | private `ViewGuard` manager |
| query | one `RequestContext`, `ResolvedScope` and `QueryWindow` |
| protocol | one bounded typed frame and direct dispatcher |

The real domain decides cancellation, deadline, idempotency, recovery and output semantics. Do not force a
read, pure transform, redb compare/set, immutable write and uncertain Qdrant mutation through the same
`OperationContext`/`MutationIdentity`/`PortReceipt` model.

## Local traits

A consumer may define a narrow local trait when it simultaneously supports real alternatives. It should:

- contain only methods the consumer calls;
- use current concrete domain values;
- carry an absolute deadline/cancellation/counters where blocking work needs them;
- return real state/readback or a domain error;
- keep its fake beside the consumer;
- disappear when only one implementation remains.

Do not rebuild the P00 registry under another name. Historical trait names, method counts, fake failure
matrices and golden source shapes do not constrain the pre-release cutover.

# Agent contract — search-control-redb

Own bounded technical control persistence. Use [root instructions](../../AGENTS.md) and the
[execution protocol](../../docs/product/EXECUTION_PROTOCOL.md) for a named owner/consumer cutover.
Missing semantic inputs go to the existing source, publication, runtime or secret owner.

## Ownership

- Journal schemas, conditional transactions, exact operation replay/recovery, route metadata,
  immutable control snapshots, quarantine records and write counters.
- Inactive source-import schema, bounded record chains, content-manifest metadata, immutable
  artifact staging/readback/publication and explicit retained/unknown cleanup disposition.
- Exact cutover marker classification and byte-stable staged-plan/commit/rollback/status projections.

Concrete redb and accepted dependencies remain private; [Cargo.toml](Cargo.toml) is the actual graph.
Use the pinned database, not another local persistence engine. [README.md](README.md) and
[FUNCTIONS.md](FUNCTIONS.md) retain current schema/profile, transaction and artifact contracts.

## Authority boundaries

Never store source bodies, extracted text, postings, vectors, term statistics or query/ranked history.
Do not infer currentness from orphaned Qdrant data. Retained-byte reads, source decryption/protection
and body hashing remain in their owners. The source adapter supplies verified digests and lengths to
the bounded content-manifest encoder; no source bytes enter control records.

Temporary-name entropy/time and native identity are injected observations, not authority. This package
classifies exact marker/publication state; the daemon decides quarantine under its root owner.
Receipt/status rendering consumes verified technical observations. It cannot authorize cutover,
invent source truth, switch a serving path or turn uncertain cleanup into success.
Preserve typed `CONTROL_STORE_CORRUPT`, `RESTORE_PENDING_REVALIDATION` and
`CONTROL_ROUTE_MISMATCH` semantics, content-free errors and private vendor types.

## Evidence inventory

Preserve these cases with their owning behavior; this is not a command list for every source slice.

| Contract | Causal evidence |
|---|---|
| Journal/profile | Required table/schema set; deterministic migration/replay; committed-only reopen; mismatched incarnation/route refusal; forbidden corpus payload rejection. |
| Hot admission | Immutable snapshots and zero durable writes, including the existing 10,000-admission case. |
| Record/content artifacts | Frozen chain/header/object/end bytes; ordering/count/byte overflow; changed second replay or native identity refusal; no clobber of an existing final locator. |
| Cutover markers | Exact replay only; different valid state never overwritten; missing/corrupt/unknown readback remain distinct and fail closed. |
| Projections | Byte-exact minimized staged-plan/cutover/status output; no duplicate daemon projection schema. |

Use the root production check/Clippy and deferred test/qualification rules. A broad historical command
in an adjacent handoff is evidence of that run, not a new source gate. Report actual reductions and
remaining consumers. Line counts are review signals; they do not require another database wrapper,
forwarding package, unrelated cleanup or a new fixture framework.

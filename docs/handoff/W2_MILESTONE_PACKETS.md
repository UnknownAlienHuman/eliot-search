# W2 bounded package milestone packets

One future writer owns one package and advances through four sequential checkpoints. These packets are
context slices, not tickets, leases, package acceptance, G1 evidence or a W2 receipt.

Machine registry: `swarm/w2-milestone-packets.toml`.

## Common checkpoint rules

- begin with failing contract, property, negative and fault tests;
- write only inside the exact package scope;
- consume exact accepted dependency handoffs, never dependency implementation source;
- keep bytes, lists, queues, retries and deadlines finite;
- preserve typed cancellation, timeout, partial, degraded and unknown outcomes;
- use stable mutation identity and exact readback after possible mutation;
- expose no secret, source body, unrestricted path or concrete vendor/runtime type;
- advance only after integration verifies checkpoint digest, raw outcomes, dependencies and line budget;
- never replay the W1 implementation packet in W2; daemon re-entry uses accepted W1 receipts only.

## Packages

- [`search-source-admission`](w2-packages/search-source-admission.md) — A0–A3.
- [`search-source-identity`](w2-packages/search-source-identity.md) — I0–I3.
- [`search-safe-reader`](w2-packages/search-safe-reader.md) — SR0–SR3.
- [`search-revision-store`](w2-packages/search-revision-store.md) — V0–V3.
- [`search-materializer`](w2-packages/search-materializer.md) — M0–M3.
- [`search-unitizer`](w2-packages/search-unitizer.md) — U0–U3.
- [`search-source-registry`](w2-packages/search-source-registry.md) — RG0–RG3.
- [`eliot-searchd`](w2-packages/eliot-searchd.md) — D20–D23.

Parallel work is allowed only between different packages whose exact dependency handoffs are accepted.
The W2 product remains DIRECT-only; Qdrant/index/query behavior belongs to later stages.

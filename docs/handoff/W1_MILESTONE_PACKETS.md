# W1 bounded package milestone packets

One future writer owns one package and advances through four sequential checkpoints. These packets are
context slices, not tickets, leases, package acceptance or a W1 receipt.

Machine registry: `swarm/w1-milestone-packets.toml`.

## Common checkpoint rules

- begin with failing contract, property, negative and fault tests;
- write only inside the exact package scope;
- keep bytes, lists, queues, retries and deadlines finite;
- preserve typed cancellation, timeout, partial, degraded and unknown outcomes;
- use stable mutation identity and exact readback after possible mutation;
- expose no secret, source body, unrestricted path or concrete vendor/runtime type;
- advance only after integration verifies checkpoint digest, raw outcomes, dependencies and line budget;
- never read Architecture Part I or dependency implementation source unless a contract challenge grants an exception.

## Packages

- [`search-config`](w1-packages/search-config.md) — C0, C1, C2, C3.
- [`search-runtime-owner`](w1-packages/search-runtime-owner.md) — R0, R1, R2, R3.
- [`search-os-secrets`](w1-packages/search-os-secrets.md) — S0, S1, S2, S3.
- [`search-control-redb`](w1-packages/search-control-redb.md) — J0, J1, J2, J3.
- [`search-provider-protocol`](w1-packages/search-provider-protocol.md) — P0, P1, P2, P3.
- [`eliot-searchd`](w1-packages/eliot-searchd.md) — D0, D1, D2, D3.
- [`eliot-search`](w1-packages/eliot-search.md) — L0, L1, L2, L3.

Parallel work is allowed only between different packages whose exact dependency handoffs are accepted.

# W3 bounded package milestone packets

One future writer owns one package and advances through four sequential checkpoints. These packets are context slices, not tickets, leases, package acceptance, Qdrant qualification, indexed-mode activation or a W3 receipt.

Machine registry: `swarm/w3-milestone-packets.toml`.

## Common checkpoint rules

- begin with failing contract, property, negative and fault tests;
- write only inside the exact package scope;
- bind exact accepted dependency/API and applicable Qdrant qualification receipts;
- keep bytes, points, batches, queues, retries and deadlines finite;
- preserve typed cancellation, timeout, partial, degraded and unknown outcomes;
- require exact acknowledgement/readback after a possible external mutation;
- expose no secret, source body, unrestricted path, raw collection/point/filter/cursor or concrete vendor/runtime type;
- advance only after integration verifies checkpoint digest, raw outcomes, dependencies, qualification state and line budget;
- never read Architecture Part I, prior-stage implementation packets or dependency implementation source unless a contract challenge grants an exception.

## Packages

- [`search-lexical`](w3-packages/search-lexical.md) — LX0–LX3.
- [`search-point-identity`](w3-packages/search-point-identity.md) — PI0–PI3.
- [`search-qdrant-supervisor`](w3-packages/search-qdrant-supervisor.md) — QS0–QS3.
- [`search-qdrant-bridge`](w3-packages/search-qdrant-bridge.md) — QB0–QB3.
- [`search-epoch-pins`](w3-packages/search-epoch-pins.md) — EP0–EP3.
- [`search-projection-planner`](w3-packages/search-projection-planner.md) — PP0–PP3.
- [`search-index-reclaimer`](w3-packages/search-index-reclaimer.md) — IR0–IR3.
- [`search-publication`](w3-packages/search-publication.md) — PUB0–PUB3.
- [`eliot-searchd`](w3-packages/eliot-searchd.md) — IDX0–IDX3.

Parallel work is allowed only between different packages whose exact dependency and qualification receipts are accepted. Indexed mode remains disabled until the separate W3 integration/qualification receipt is accepted.

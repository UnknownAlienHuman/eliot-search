# W9 Product Pulse and Windows qualification

This directory is the bounded P15/G5 implementation projection for ELIOT Search 8.4.
It defines how the baseline product is measured and accepted; it contains no runtime implementation and
no acceptance claim.

- [`W9_PRODUCT_PULSE_CONTRACTS_1.0.md`](W9_PRODUCT_PULSE_CONTRACTS_1.0.md) — cross-package experiment,
  evidence and verdict semantics.
- [`manifest.toml`](manifest.toml) — machine ownership and invariant inventory.
- [`../../qualification/product-pulse/`](../../qualification/product-pulse/README.md) — control corpus,
  metric registry, mandatory probes and G5 evidence map.
- [`../../config/w9-product-pulse.toml`](../../config/w9-product-pulse.toml) — evaluation-only locked and
  bounded settings.
- [`../../swarm/w9-product-pulse.toml`](../../swarm/w9-product-pulse.toml) — one-package agent packet and
  integration-owner prerequisites.

`search-eval` owns schemas, deterministic aggregation, leakage audits and verdict construction. The
integration owner executes the product through accepted public/provider interfaces and supplies immutable
raw evidence. A package writer cannot accept its own gate.

W9 remains blocked until G4 and the lifecycle/security hardening receipts required by the W9 packet are
accepted. Optional semantic/document/scale depth remains blocked until an independently reviewed P15
`ACCEPTED` verdict exists.

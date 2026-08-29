# Dependency graph and launch topology

`swarm/crates.toml` is the machine authority. This file explains semantic topology and the sole
progressive-composition exception.

## Foundation

```text
search-contracts
  ├─ search-domain
  └─ search-ports
```

`search-contracts` is accepted first. `search-domain` and `search-ports` may then run in parallel from
the same immutable contracts API/schema digest.

## Capability waves

```text
W1  runtime-owner, os-secrets, control-redb, provider-protocol shell
W2  source-admission, source-identity, registry, safe-reader, revision-store,
    text/code materializer and unitizer
W3  point identity, projection planner, Qdrant supervisor/bridge, lexical encoder,
    publication, epoch pins and index reclaimer
W4  access, query planner/executor, candidate validator, handles, projector,
    continuation and evaluation baseline
W5  reconciler, overlay and Rust code enrichment
W6  exact proof, subject resolver and comparator
W7  retention/purge/restore and security/lifecycle hardening
W8  generic edge hardening and optional ELIOT/Research leaf adapters
W9  Product Pulse and Windows qualification
W10 optional model/document/advanced-scale work after accepted P15 + ADR
```

Packages in one wave are not automatically parallel. Every direct dependency and consumed port must
have an accepted commit and API digest first. Exact dependency lists live only in
`swarm/crates.toml`; Markdown does not duplicate them.

## Adapter direction

```text
search-contracts records
        ↓
search-ports traits ← pure search-domain rules where applicable
        ↓
capability/orchestration package
        ↓
concrete adapter implementation
        ↓
eliot-searchd composition
```

A Qdrant startup example:

```text
SecretStorePort → search-os-secrets → bounded SecretLease
ProcessSupervisorPort → search-qdrant-supervisor consumes lease
SearchIndexPort → search-qdrant-bridge owns data plane
eliot-searchd constructs and connects all three
```

No concrete adapter opens another adapter's private store or exposes a vendor type through a port.

## Progressive daemon composition

```text
wave1-shell
  contracts + domain + ports + runtime-owner + os-secrets + control-redb + provider-protocol
wave2-source
  + source admission/identity/registry/readback/revision/materialization/unitization
wave3-index
  + lexical + point/projection + Qdrant supervisor/bridge + publication + pins/reclaimer
wave4-query
  + access/planner/executor/validator + handles/projector/continuation + eval baseline
wave5-current
  + reconciler/overlay/code enrichment
wave6-proof
  + exact/subject/comparator
wave7-lifecycle
  + retention/purge/restore hardening
```

`eliot-searchd` is first launched at W1 but declares feature-gated optional dependencies from later
waves. `progressive_composition = true` is the only wave-monotonicity exception; a feature cannot be
enabled without accepted dependency receipts.

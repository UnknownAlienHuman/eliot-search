# Dependency graph and launch topology

The package graph is acyclic. `swarm/crates.toml` is machine authority; this document explains launch
and port semantics that Cargo alone cannot express.

## Foundation

```text
search-contracts
└─ search-domain
```

`search-contracts` is implemented and accepted first. `search-domain` starts only from its accepted
public contract digest.

## Capability graph

```text
W1
  search-runtime-owner
  search-os-secrets
  search-control-redb
  search-provider-protocol (frame/session shell)

W2
  search-source-admission
  search-source-identity
  search-source-registry        ← identity + admission
  search-safe-reader
  search-revision-store
  search-materializer
  search-unitizer

W3
  search-point-identity
  search-projection-planner     ← point-identity
  search-qdrant-supervisor      (process port; daemon supplies secret lease)
  search-qdrant-bridge          (data plane only)
  search-lexical
  search-epoch-pins
  search-publication            ← planner + point-identity; journal/index via ports
  search-index-reclaimer        ← epoch-pins + committed retired manifest

W4
  search-access
  search-query-planner          ← access
  search-retrieval-executor     ← planner + lexical + pins + access; index via port
  search-candidate-validator    ← access; source readback via port
  search-handles
  search-result-projector       ← validator + handles
  search-continuation           ← planner + access + pins
  search-eval (baseline harness)

W5
  search-source-reconcile       ← registry + identity + safe-reader
  search-overlay                ← unitizer + lexical
  search-code-enricher

W6
  search-exact                  ← access; inventory/readback via ports
  search-subject-resolver
  search-comparator             ← subject-resolver

W7
  search-retention              ← pins + reclaimer + handles; stores/index via ports
  hardening passes reuse access, validator, continuation, publication and revision store

W8
  search-eliot-adapter          ← provider-protocol
  search-research-export-adapter← provider-protocol
  provider-protocol binding/integration hardening

W9
  search-eval owns Product Pulse and Windows qualification evidence

W10
  model/document/advanced-scale packages only after accepted P15 + dedicated ADR
```

Packages in one wave are not automatically parallel. Every direct dependency and consumed port must
have an accepted handoff/API digest before the consumer starts.

## Adapter direction

```text
capability/orchestration
  → vendor-neutral port
  → daemon binding
  → concrete adapter

SearchIndexPort        → search-qdrant-bridge
ProcessSupervisorPort  → search-qdrant-supervisor
ControlJournalPort     → search-control-redb
SecretStorePort        → search-os-secrets
SourceRevisionStorePort→ search-revision-store
```

For Qdrant startup, daemon composition resolves a purpose/incarnation-bound secret lease through
`SecretStorePort` and supplies that lease to `ProcessSupervisorPort`. The two concrete adapters never
open each other's stores or types.

Concrete adapters do not appear in query/lifecycle public APIs. Vendor, OS and database types never
travel back into contracts/domain or client adapters.

## Progressive daemon composition

```text
wave1-shell
  contracts + domain + runtime-owner + os-secrets + control-redb + provider-protocol

wave2-source
  + admission + identity/registry + safe reader + revision/materializer/unitizer

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
waves. `progressive_composition = true` is the sole wave-monotonicity exception; no later feature may be
enabled without accepted dependency receipts.

# Dependency graph and launch topology

The package graph is acyclic. `swarm/crates.toml` is machine authority; this document explains the
launch semantics that are easy to miss from Cargo alone.

## Foundation

```text
search-contracts
└─ search-domain
```

`search-contracts` is implemented and accepted first. `search-domain` may start only from the accepted
public contract digest, not from a concurrently changing contracts worktree.

## Capability graph

```text
W1
  search-runtime-owner  ← contracts, domain
  search-control-redb   ← contracts, domain
  search-provider-protocol (frame/session shell) ← contracts, domain

W2
  search-source-identity
  search-source-registry        ← source-identity
  search-safe-reader
  search-revision-store
  search-materializer
  search-unitizer

W3
  search-point-identity
  search-projection-planner     ← point-identity
  search-qdrant-bridge
  search-lexical
  search-epoch-pins
  search-publication            ← planner, point-identity, qdrant-bridge, epoch-pins

W4
  search-access
  search-query-planner          ← access
  search-retrieval-executor     ← planner, bridge, lexical, epoch-pins, access
  search-candidate-validator    ← access, revision-store
  search-result-projector       ← candidate-validator
  search-continuation           ← planner, access, epoch-pins
  search-eval (baseline harness)

W5
  search-source-reconcile       ← registry, identity, safe-reader
  search-overlay                ← unitizer, lexical
  search-code-enricher

W6
  search-exact                  ← registry, revision-store, safe-reader, access
  search-subject-resolver
  search-comparator             ← subject-resolver

W7
  search-retention              ← control-redb, revision-store, qdrant-bridge, epoch-pins
  hardening passes reuse search-access, candidate-validator and continuation

W8
  search-eliot-adapter          ← provider-protocol
  search-research-export-adapter← provider-protocol
  provider-protocol binding/integration hardening

W9
  search-eval owns Product Pulse and Windows qualification evidence

W10
  search-model-provider and model worker only after accepted P15 + ADR
  document worker/provider depth only after accepted P15 + ADR
```

Packages listed in the same wave are not automatically parallel. A writer starts only when every direct
dependency handoff is accepted. Examples: `search-source-identity` precedes `search-source-registry`;
`search-point-identity` precedes `search-projection-planner`; `search-access` precedes
`search-query-planner`; `search-subject-resolver` precedes `search-comparator`.

## Adapter direction

Vendor/client adapters terminate at their boundary:

```text
query capability → vendor-neutral port → search-qdrant-bridge → qdrant-client
generic provider protocol → optional leaf adapter → external client contract
```

Qdrant, redb, Windows and client-system types do not travel back into contracts/domain or across public
package ports.

## Progressive daemon composition

The final daemon references many packages, but it is not implemented as one all-context task.

```text
wave1-shell
  contracts + domain + runtime-owner + control-redb + provider-protocol

wave2-source
  + source identity/registry/safe reader/revision/materializer/unitizer

wave3-index
  + lexical/point identity/projection/bridge/publication/pins

wave4-query
  + access/planner/executor/validator/projector/continuation/eval baseline

wave5-current
  + reconciler/overlay/code enrichment

wave6-proof
  + exact/subject/comparator

wave7-lifecycle
  + retention/purge/restore hardening
```

Only the active feature layer and accepted dependency handoffs enter the daemon writer's context.

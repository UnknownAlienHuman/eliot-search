# docs

| Directory | Contents |
|---|---|
| `architecture/` | The authoritative Search architecture master. One file; no second normative document. |
| `handoff/` | Implementation handoff and the PR delivery graph derived from the architecture. |
| `adr/` | Architecture Decision Records. Required for any load-bearing default, new owner, vendor selection or contract change. |
| `contracts/` | Hand-written contract notes not yet generated. |
| `generated/` | Generated projections: schemas, reason-code registry, capability descriptor, command surface. Never hand-edited. |

Prose here explains rationale, owners and failure behavior. Schemas, registries and matrices move into
`generated/` as implementation lands.

# Documentation

| Directory | Contents |
|---|---|
| `architecture/` | The single authoritative ELIOT Search 8.4 architecture, handoff and audit master. |
| `adr/` | Architecture Decision Records for load-bearing defaults, package boundaries and vendor choices. |
| `handoff/` | Swarm execution protocol, package matrix and implementation waves derived from Architecture 8.4. |
| `contracts/` | Hand-written contract notes not yet generated. |
| `generated/` | Generated schemas, registries and descriptors. Never hand-edit. |

Ordinary package agents read the nearest `AGENTS.md`, not the full architecture master. The master
remains the source of truth; package instructions are bounded implementation extracts. Machine-readable
assignment metadata lives in [`../swarm/crates.toml`](../swarm/crates.toml).

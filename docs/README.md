# Documentation

| Directory | Contents |
|---|---|
| `architecture/` | The single authoritative ELIOT Search 8.4 architecture, handoff and audit master. |
| `adr/` | Architecture Decision Records for load-bearing defaults, package boundaries and vendor choices. |
| `handoff/` | Swarm readiness audit, package matrix, dependency topology, implementation waves and P00 bootstrap. |
| `contracts/` | Hand-written contract notes not yet generated. |
| `generated/` | Generated schemas, registries and descriptors. Never hand-edit. |

Ordinary package agents read root/family/package `AGENTS.md`, `../swarm/ASSIGNMENT_PROTOCOL.md`, one file
from [`../swarm/assignments/`](../swarm/assignments/README.md), and accepted public dependency handoffs.
They do not load the architecture master during ordinary work.

Machine-readable package metadata lives in [`../swarm/crates.toml`](../swarm/crates.toml). Current
authorization lives separately in [`../swarm/launch-state.toml`](../swarm/launch-state.toml).

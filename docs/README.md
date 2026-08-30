# docs

| Directory | Contents |
|---|---|
| `architecture/` | Authoritative single-file Architecture 8.4 implementation master. |
| `contracts/p00/` | Compact field-level W0 projection, canonicalization, recipes, reasons and ports. |
| `config/` | Configuration format, section ownership and composite reconfiguration contracts. |
| `current/` | W5 observation continuity, saved/unsaved overlay and Rust syntax contracts. |
| `client/` | Generic provider edge, standalone client and optional leaf-profile contracts. |
| `evaluation/` | Product Pulse, Windows qualification, corpus/metric and verdict contracts. |
| `optional/` | Post-P15 model, document, advanced-scale and candidate-evaluation contracts. |
| `handoff/` | Package/stage packets, stage-specific read-set resolver, dependency/ownership maps and readiness audits. |
| `adr/` | Load-bearing implementation/package decisions. |
| `generated/` | Generated schemas/registries/descriptors after P00; never hand-edited. |

The stage-context entry points are:

```text
handoff/SWARM_LAUNCH_INDEX.md
handoff/SWARM_STAGE_READSETS.md
handoff/STAGE_READSET_AUDIT.md
```

Their machine sources are repository-root `swarm/stages.toml` and `swarm/stage-readsets.toml`. They
describe future bounded context only; `swarm/launch-state.toml` remains the sole current authorization.

External-artifact, provider and product-evidence qualification inputs live under repository-root
`qualification/`; they remain unaccepted until exact executed evidence and an independent reviewer
receipt exist.

Part I of the architecture master remains normative. Contract/configuration/qualification/stage/read-set
projections are derivative bounded-agent inputs and stop on contradiction. None of them authorizes a
package, provider, topology, runtime behavior, Product Pulse or optional-depth acceptance.

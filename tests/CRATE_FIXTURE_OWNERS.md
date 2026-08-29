# Shared fixture ownership

Package-local unit/property/fault fixtures live with the owning package. Shared fixtures are changed
only by the owner below; other writers submit a typed request.

| Shared area | Owner | Consumers |
|---|---|---|
| control corpus and Product Pulse data | `search-eval` | product/acceptance packages |
| contract serialization, recipes and reason registries | `search-contracts` | every package |
| shared port operation/conformance fakes | `search-ports` | capabilities, adapters, daemon |
| pure invariant/property generators | `search-domain` | capability packages |
| OS-secret binding/rotation/side-channel fixtures | `search-os-secrets` | daemon, supervisor, protocol |
| redb migration/corruption fixtures | `search-control-redb` | runtime, publication, retention |
| source-admission deny-by-default corpus | `search-source-admission` | registry, reader, eval |
| source identity/path history fixtures | `search-source-identity` | registry, reconcile, exact |
| no-execute/reparse/stable-read corpus | `search-safe-reader` | source, exact, validator |
| anchor/coordinate/revision fixtures | `search-revision-store` | materializer, exact, validator |
| lexical golden vectors | `search-lexical` | projection, executor, eval |
| Qdrant process/ACL/Job Object fixtures | `search-qdrant-supervisor` | daemon, P05 qualification |
| Qdrant capability/schema/data-plane fixtures | `search-qdrant-bridge` | projection, publication, executor, reclaimer |
| point-collision fixture | `search-point-identity` | projection, publication |
| publication kill/failpoint matrix | `search-publication` | daemon, reclaimer, retention, eval |
| epoch/route pin and watermark fixtures | `search-epoch-pins` | continuation, reclaimer, retention |
| exact retired-point reclaim fixtures | `search-index-reclaimer` | publication, retention, eval |
| source-handle TTL/auth/revocation fixtures | `search-handles` | projector, retention, adapters |
| provider envelope/framing fixtures | `search-provider-protocol` | CLI, daemon, adapters |
| optional ELIOT mapping fixture | `search-eliot-adapter` | P14 integration |
| optional normalized-bundle fixture | `search-research-export-adapter` | P14 integration |

Contribution preparation may be delegated, but semantic review and merge receipt stay with the owner.

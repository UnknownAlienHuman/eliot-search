# Shared fixture ownership

Package-local unit, property and fault fixtures live in the owning package. Shared fixtures are edited
only by the owner below; other writers submit a fixture request with the exact invariant and expected
artifact.

| Shared area | Owner | Consumers |
|---|---|---|
| `tests/control-corpus/` | `search-eval` | all product and acceptance packages |
| contract serialization and recipe fixtures | `search-contracts` | every package |
| pure invariant/property generators | `search-domain` | capability packages |
| OS-secret binding, rotation and side-channel fixtures | `search-os-secrets` | daemon, Qdrant supervisor, provider protocol |
| redb migration/corruption fixtures | `search-control-redb` | runtime, publication, retention |
| source-admission policy and deny-by-default corpus | `search-source-admission` | registry, safe reader, eval |
| source identity/path history fixtures | `search-source-identity` | registry, reconcile, exact |
| no-execute/reparse/stable-read corpus | `search-safe-reader` | source, exact, validator |
| anchor/coordinate/revision fixtures | `search-revision-store` | materializer, exact, validator |
| lexical golden vectors | `search-lexical` | projection, executor, eval |
| Qdrant executable/process/ACL/Job Object fixtures | `search-qdrant-supervisor` | daemon, P05 qualification |
| Qdrant capability/schema/data-plane fixtures | `search-qdrant-bridge` | projection, publication, executor, reclaimer |
| point-collision fixture | `search-point-identity` | projection, publication |
| publication kill/failpoint matrix | `search-publication` | daemon, reclaimer, retention, eval |
| epoch/route pin and watermark fixtures | `search-epoch-pins` | continuation, reclaimer, retention |
| exact retired-point reclaim fixtures | `search-index-reclaimer` | publication, retention, eval |
| source-handle mint/TTL/authorization/revocation fixtures | `search-handles` | projector, retention, adapters |
| provider envelope/framing fixtures | `search-provider-protocol` | CLI, daemon, adapters |
| optional ELIOT mapping fixture | `search-eliot-adapter` | P14 integration |
| optional normalized-bundle fixture | `search-research-export-adapter` | P14 integration |

A fixture owner may accept a contribution prepared by another agent, but semantic review and merge
receipt remain with the owner. No writer edits the shared control corpus opportunistically.

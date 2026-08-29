# Shared fixture ownership

Package-local unit, property and fault fixtures live in the owning package. Shared fixtures are edited
only by the owner below; other writers submit a fixture request with the exact invariant and expected
artifact.

| Shared area | Owner | Consumers |
|---|---|---|
| `tests/control-corpus/` | `search-eval` | all product and acceptance packages |
| contract serialization and recipe fixtures | `search-contracts` | every package |
| pure invariant/property generators | `search-domain` | capability packages |
| redb migration/corruption fixtures | `search-control-redb` | runtime, publication, retention |
| source identity/path/no-execute corpus | `search-safe-reader` with `search-source-identity` review | source, exact, validator |
| anchor/coordinate/revision fixtures | `search-revision-store` | materializer, exact, validator |
| lexical golden vectors | `search-lexical` | projection, executor, eval |
| Qdrant capability/schema fixture | `search-qdrant-bridge` | projection, publication, executor |
| point-collision fixture | `search-point-identity` | projection, publication |
| publication kill/failpoint matrix | `search-publication` | runtime, retention, eval |
| provider envelope/framing fixtures | `search-provider-protocol` | CLI, daemon, adapters |
| optional ELIOT mapping fixture | `search-eliot-adapter` | P14 integration |
| optional normalized-bundle fixture | `search-research-export-adapter` | P14 integration |

A fixture owner may accept a contribution prepared by another agent, but the merge and semantic receipt
remain with the owner. No writer edits the shared control corpus opportunistically.

# Swarm readiness audit — 2026-08-29

## Verdict

The repository is structurally ready to assign `search-contracts` for P00/W0. It is not an implemented
product and all W1+ packages remain blocked.

Current scaffold: **40 library packages + 4 binaries = 44 one-writer packages**.

## Closed findings

| ID | Severity | Finding | Closure |
|---|---:|---|---|
| F-01 | Blocker | Handle creation/expansion/revocation lacked one state owner. | Added `search-handles`. |
| F-02 | Blocker | Pin watermarks had no ordinary exact-ID deletion owner. | Added `search-index-reclaimer`. |
| F-03 | Major | Qdrant process containment and data plane were conflated. | Split supervisor and bridge. |
| F-04 | Major | OS-bound credentials had no reusable owner. | Added `search-os-secrets`. |
| F-05 | Major | Source admission was duplicated by registry/reader. | Added `search-source-admission`. |
| F-06 | Blocker | Orchestration depended on concrete adapters. | Port-only orchestration; daemon composition. |
| F-07 | Major | Ordinary reclaim, CAS retention and security purge could collapse. | Separate owners and receipts. |
| F-08 | Major | Qdrant supervisor linked directly to secret adapter. | Bounded secret lease through ports. |
| F-09 | Blocker | H4 ports had no Cargo owner; agents would invent local traits. | Added `search-ports`. |
| F-10 | Blocker | P00 assignments lacked exact field-level schemas and reason governance. | Added hash-receipted P00 contract pack. |
| F-11 | Blocker | `SourceOwnerGeneration` conflicted between Part I and H3.1. | Part I wins: BLAKE3-256; ADR 0003 records correction. |
| F-12 | Major | Cargo and registry dependency lists could drift. | Registry is machine authority; actual manifests wired to `search-ports`; Markdown stops duplicating deps. |

## Mechanical acceptance required before merge

- Cargo members, package directories, registry entries and assignments are identical: 44;
- every registry dependency exists and the graph is acyclic;
- ordinary packages never depend on a later earliest wave;
- daemon is the sole feature-gated progressive-composition exception;
- Cargo manifests and registry dependency sets match;
- new source files are boundary placeholders only;
- P00 pack has no unresolved contradiction with Part I;
- launch state authorizes contracts only and keeps W1+/optional depth blocked.

## Still unproved

- exact stable Rust/dependency set and `Cargo.lock`;
- contract/domain/port implementation and generated schemas;
- Qdrant server/client artifact qualification;
- Windows process, ACL and secret-store behavior;
- redb/CAS/Qdrant fault correctness;
- access noninterference, latency/resource targets and Product Pulse.

## Launch recommendation

Assign `search-contracts`; accept its canonical API/schema digest; then assign `search-domain` and
`search-ports` separately. The integration owner publishes W0 evidence before advancing launch state.

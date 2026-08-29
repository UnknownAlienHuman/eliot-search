# Swarm readiness audit — 2026-08-29

## Verdict

The repository is structurally ready to assign `search-contracts` for P00/W0. It is not an implemented
product and all W1+ packages remain blocked.

Current scaffold: **41 library packages + 4 binaries = 45 one-writer packages**.

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
| F-09 | Blocker | Shared ports had no Cargo owner; agents would invent local traits. | Added `search-ports`. |
| F-10 | Blocker | P00 assignments lacked exact field-level schemas/reason governance. | Added hash-receipted P00 pack. |
| F-11 | Blocker | `SourceOwnerGeneration` conflicted between Part I and a handoff sketch. | Part I wins: BLAKE3-256. |
| F-12 | Major | Cargo and registry dependency lists could drift. | Exact dependency parity is mechanically checked. |
| F-13 | Blocker | `search-config` existed in Cargo while registry, launch counts and human matrix still reported 44 packages. | Registry/launch/matrix synchronized to 45 packages and daemon composition wired to `search-config`. |
| F-14 | Major | Configuration sections had no bounded package-owner packets and the example omitted sections. | Added twenty section packets plus one complete safe DIRECT example. |
| F-15 | Major | Reconfiguration was sketched as one scalar severity, which could erase concurrent security/restart/rebuild obligations. | Composite reconfiguration actions preserve every required obligation. |
| F-16 | Blocker | W3 crates required agents to infer operation recovery/idempotency and Qdrant qualification from the master. | Added package-local W3 `FUNCTIONS.md` files and unqualified P05–P07 evidence packets. |
| F-17 | Major | W4 function/qualification files existed but were not registered as bounded package read sets. | Registry schema v6 links all W4 query/protocol/eval packages to exact `FUNCTIONS.md` and W4 qualification paths. |
| F-18 | Blocker | W5 agents would need to infer observation continuity, overlay shadowing and Rust assurance from the architecture master. | Added complete package-local W5 function contracts, implementation packet and machine probe corpus. |
| F-19 | Blocker | Unsaved-byte non-persistence was a prose invariant without an exhaustive sink audit. | W5 qualification requires independent probes for redb/CAS/Qdrant, logs/metrics/traces, backup/restore/crash, provider cache/eval/training and restart invalidation. |
| F-20 | Major | Tolerant Rust syntax could be overclaimed as compiler truth or execute repository tooling. | W5 baseline locks no-execute behavior, exact artifact qualification, cfg variant preservation and `tolerant_syntax` assurance. |

## Mechanical acceptance required before merge

- Cargo members, package directories, registry entries and assignments are identical: 45;
- library/binary counts are 41/4 across registry, launch state and human matrix;
- every registry dependency exists and the graph is acyclic;
- Cargo manifests and registry dependency sets match;
- every registry-declared function/configuration/qualification packet exists;
- all W4 query packages are linked to W4 qualification and all three W5 packages to W5 qualification;
- W5 baseline stays `DESIGNED_NOT_EXECUTED`, parser/provider identities stay `UNSELECTED` and all 42
  mandatory probes stay `UNAVAILABLE` before execution;
- observation currentness cannot be inferred from watcher quietness or incomplete inventory;
- unsaved persistence, stale-base fallback and durable unsaved handles remain structurally forbidden;
- Rust enrichment cannot execute build/proc-macro/LSP/shell/network behavior or claim compiler truth;
- G3 gate requires currentness recovery, live-head shadow, overlay non-persistence/restart invalidation and
  qualified no-execute Rust evidence;
- daemon is the sole feature-gated progressive-composition exception;
- new source files remain boundary placeholders only;
- launch state authorizes contracts only and keeps W1+/optional depth blocked.

## Still unproved

- exact stable Rust/dependency set and committed `Cargo.lock`;
- contract/domain/port/configuration implementation and generated schemas;
- exact Qdrant server/client artifact selection and probe execution;
- lexical golden vectors and collision thresholds;
- Windows process, ACL, Job Object and secret-store behavior;
- redb/CAS/Qdrant fault correctness;
- publication failpoint execution and exact reclaim runtime proof;
- Windows watcher/USN and IDE buffer adapter behavior;
- exhaustive unsaved-byte sink audit;
- exact Rust parser/grammar/profile qualification and no-execute runtime proof;
- access noninterference, current-workspace latency/resource targets and Product Pulse.

## Launch recommendation

Assign `search-contracts`; accept its canonical API/schema digest; then assign `search-domain` and
`search-ports` separately. `search-config` remains W1-blocked. W3–W5 packets are preparation inputs only;
they do not authorize Qdrant, query, current-workspace, overlay or parser implementation before accepted
preceding handoffs and explicit package tickets.

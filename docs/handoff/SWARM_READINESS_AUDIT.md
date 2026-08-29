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
| F-17 | Major | W4 function/qualification files existed but were not registered as bounded package read sets. | Registry links all W4 query/protocol/eval packages to exact function and qualification paths. |
| F-18 | Blocker | W5 agents would need to infer observation continuity, overlay shadowing and Rust assurance from the architecture master. | Added complete package-local W5 function contracts, implementation packet and machine probe corpus. |
| F-19 | Blocker | Unsaved-byte non-persistence was a prose invariant without an exhaustive sink audit. | W5 qualification requires independent probes for storage, diagnostics, backup/crash, provider/eval/training and restart invalidation. |
| F-20 | Major | Tolerant Rust syntax could be overclaimed as compiler truth or execute repository tooling. | W5 baseline locks no-execute behavior, exact artifact qualification, cfg variant preservation and `tolerant_syntax` assurance. |
| F-21 | Blocker | W6 resolver could silently choose one same-name/top-ranked candidate when evidence was materially ambiguous. | Resolver function contract fixes ladder order, coherent fences, bounded ambiguity and drift revalidation; ambiguity is a normal output. |
| F-22 | Major | Comparison could overcount forks, collapse cfg variants or produce a hidden correctness/adoption verdict. | Comparator contract separates lineage, evidence roles, configuration applicability, coverage and non-normative output. |
| F-23 | Blocker | Exact negative proof could be implemented over Qdrant/top-k or omit unreadable/drifted/cancelled items. | Exact contract freezes authoritative inventory denominator and requires one result/failure per item; every incomplete condition blocks complete negative. |
| F-24 | Major | Exact regex/structural semantics and checkpoint recovery had no qualification corpus. | Added unselected profile registry, safe-engine requirements, 52 mandatory probes and checkpoint readback/resume evidence. |

## Mechanical acceptance required before merge

- Cargo members, package directories, registry entries and assignments are identical: 45;
- library/binary counts are 41/4 across registry, launch state and human matrix;
- every registry dependency exists and the graph is acyclic;
- Cargo manifests and registry dependency sets match;
- every registry-declared function/configuration/qualification packet exists;
- W6 resolver/comparator/exact entries bind exact `FUNCTIONS.md` and `qualification/proof/W6_QUALIFICATION.md`;
- registry schema is 7 and launch state references it while staying P00/W0;
- W6 baseline stays `DESIGNED_NOT_EXECUTED`, proof profiles stay `UNQUALIFIED`, regex/structural provider identities stay `UNSELECTED` and all 52 probes stay `UNAVAILABLE`;
- resolver cannot fall through a failed explicit reference, mix fences or resolve after an incomplete higher ladder;
- comparator cannot inflate forks, treat tests/docs as truth, collapse mutually exclusive cfg variants or emit a normative verdict;
- exact plane cannot use Qdrant/top-k/client lists as denominator, substitute current path revisions or allow any incomplete condition to satisfy complete negative proof;
- G3 has separate evidence IDs for subject drift, lineage/cfg variants, non-normative coverage, predicate qualification, frozen denominator, incomplete failures and security/unsaved revalidation;
- daemon remains the sole feature-gated progressive-composition exception;
- new source files remain boundary placeholders only;
- launch state authorizes contracts only and keeps W1+/optional depth blocked.

## Still unproved

- exact stable Rust/dependency set and committed `Cargo.lock`;
- contract/domain/port/configuration implementation and generated schemas;
- exact Qdrant server/client artifact selection and probe execution;
- Windows watcher/USN and IDE buffer adapter behavior;
- exhaustive unsaved-byte sink audit;
- exact Rust parser/grammar/profile qualification and no-execute runtime proof;
- resolver behavior on real repository ambiguity/rename/overload corpora;
- repository lineage/copy independence and cfg-aware comparison runtime;
- exact regex engine/source/license/performance qualification;
- frozen inventory/revision/access/overlay exact executor and checkpoint fault behavior;
- complete-negative proof under real unreadable/drift/cancel/security faults;
- access noninterference, latency/resource targets and Product Pulse.

## Launch recommendation

Assign `search-contracts`; accept its canonical API/schema digest; then assign `search-domain` and
`search-ports` separately. `search-config` remains W1-blocked. W3–W6 packets are preparation inputs only;
they do not authorize index, query, current-workspace, comparison or exact-proof implementation before
accepted preceding handoffs and explicit package tickets.

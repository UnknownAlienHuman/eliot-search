# Swarm readiness audit — 2026-08-29

## Verdict

The repository is structurally ready to assign `search-contracts` for P00/W0. It is not an implemented
product and every W1+ package remains blocked.

Current scaffold: **41 library packages + 4 binaries = 45 one-writer packages**.

The bounded architecture/qualification surface covers W0 through W10. Coverage means future agents have
package-scoped contracts, stage-specific replacement contexts and hard stop conditions; it is not
evidence that any runtime, provider, performance or product gate has passed.

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
| F-13 | Blocker | `search-config` existed in Cargo while registries still reported 44 packages. | Registry/launch/matrix synchronized to 45 packages. |
| F-14 | Major | Settings sections lacked bounded package-owner packets. | Added twenty section packets and a complete safe DIRECT example. |
| F-15 | Major | One scalar reconfiguration severity could erase concurrent obligations. | Composite action sets preserve barriers, restart, generation, rebuild and gate steps. |
| F-16 | Blocker | W3 crates had to infer recovery/idempotency and Qdrant qualification. | Added package-local W3 functions and P05–P07 evidence packets. |
| F-17 | Major | W4 query/protocol/eval files were not registered as bounded read sets. | Registered exact function/qualification paths. |
| F-18 | Blocker | W5 agents had to infer observation, overlay and Rust assurance. | Added complete W5 functions, settings and qualification corpus. |
| F-19 | Blocker | Unsaved non-persistence lacked an exhaustive sink audit. | Required storage/diagnostic/backup/crash/provider/eval/training and restart probes. |
| F-20 | Major | Tolerant Rust syntax could be overclaimed or execute repository tooling. | Locked no-execute behavior, exact parser qualification and `tolerant_syntax` assurance. |
| F-21 | Blocker | Resolver could choose same-name/top-ranked candidate despite ambiguity. | Fixed ladder order, coherent fences, bounded ambiguity and drift revalidation. |
| F-22 | Major | Comparison could inflate forks/cfg variants or emit a hidden verdict. | Separated lineage, evidence roles, applicability, coverage and descriptive output. |
| F-23 | Blocker | Exact negative proof could use Qdrant/top-k or omit incomplete items. | Frozen authoritative inventory denominator and one outcome per item. |
| F-24 | Major | Exact regex/structural semantics and checkpoint recovery lacked qualification. | Added unselected profiles, safe-engine floors and 52 W6 probes. |
| F-25 | Blocker | Purge, restore, durable handles and CAS sweep lacked one cross-package hardening packet. | Added W7 security/lifecycle contracts with monotonic fences, truthful receipts and non-resurrection. |
| F-26 | Blocker | Generic clients/adapters could gain hidden store, grant, memory or finish authority. | Added authenticated W8 generic edge, standalone CLI and disabled leaf adapters with authority guards. |
| F-27 | Blocker | Product acceptance could be declared from unit tests, ad-hoc benchmarks or post-hoc thresholds. | Added W9 Product Pulse: 49 cases, 33 metrics, 60 G5 probes, frozen A/B/C policy and independent review. |
| F-28 | Blocker | Optional model/document/scale packages could self-select a provider or activate from config/feature presence. | Added candidate-specific W10 gate, exact profile/artifact qualification, benefit, removal and migration/rollback contracts. |
| F-29 | Major | Optional model output could become evidence, widen rerank scope or silently replace another provider. | Model contract restricts output to nomination/ranking; rerank is subset-only and fallback is explicit. |
| F-30 | Blocker | Document providers could execute active content, follow remote resources or overclaim coordinates/fidelity. | Isolated no-execute/no-network worker, bomb/path containment and coordinate/loss-map validation. |
| F-31 | Blocker | Advanced scale could mutate active topology in place or treat a Qdrant alias as commit. | P18 requires measured bottleneck, candidate generation, R0/catch-up/R1, guarded redb switch, pin drain and rollback. |
| F-32 | Major | Optional removal could stop a worker without restoring accepted baseline behavior. | Baseline route/config restore precedes drain/reclaim and requires P15 regression plus cleanup receipt. |
| F-33 | Blocker | Progressive packages would reread all historical stage documents or infer later obligations from old packets. | Added exact W0–W10 stage registry and 23 replacement read sets over immutable accepted handoffs. |
| F-34 | Major | W8 CLI and W10 candidate evaluation had no narrow package reentry deltas. | Added `W8_CLIENT.md`, `W10_OPTIONAL_EVALUATION.md` and machine/validator integration. |

## Current bounded packet inventory

| Slice | Package/qualification status |
|---|---|
| W0 / P00 | Exact contract/domain/port projection; only `search-contracts` currently authorized. |
| W1–W2 / P01–P04 | Function/config packets prepared; runtime/source implementation blocked. |
| W3 / P05–P07 | Qdrant/lexical/publication/pins/reclaim packets prepared; artifacts/probes unqualified. |
| W4 / P08 | Query/access/validation/result/handle/protocol packets prepared; runtime probes unavailable. |
| W5 / P09–P10 | Currentness/overlay/Rust syntax packets prepared; parser and evidence unselected. |
| W6 / P11–P12 | Resolver/comparator/exact-proof packets prepared; regex/structural profiles unqualified. |
| W7 / P13 | Security/lifecycle hardening packet prepared; purge/restore/runtime evidence absent. |
| W8 / P14 | Generic edge/standalone CLI/leaf adapter packet prepared; optional leaf profiles disabled. |
| W9 / P15 | Product Pulse contract prepared; corpus/baselines/Windows/policy unselected, 60 probes unavailable. |
| W10 / P16–P18 | Model/document/scale/evaluation packets prepared; all profiles disabled/unselected, G6 not accepted. |

## Stage/read-set closure

Machine state is now:

```text
packages:                         45
stages:                           11 (W0–W10)
stage-package assignments:        68
later-stage replacement contexts: 23
unique reused packages:           13
maximum static context:           16 files
current launch:                    P00 / W0
```

Every package first appears at its exact registry wave. A later assignment has one replacement override;
an earliest-wave assignment has none. Previous implementation packets and dependency source are replaced
by immutable accepted public handoffs. W8 CLI and W10 candidate evaluation have package-local deltas.

## Mechanical acceptance requirements

- Cargo members, package directories, registry entries and assignments remain exactly 45;
- library/binary counts remain 41/4 and the dependency graph remains acyclic;
- Cargo manifests and registry direct dependencies remain equal;
- package/function/stage/read-set registries close over identical package identity/write ownership;
- stages remain exactly W0–W10 with 68 assignments;
- every later assignment has one of 23 exact overrides and no base assignment has one;
- all active stage context excludes prior implementation packets, architecture and dependency internals;
- static context is at most sixteen files;
- launch authority remains P00/W0 with only `search-contracts` authorized;
- optional-depth launch section points to the W10 packet/qualification/settings and selects no candidate;
- all optional flags are false and model/document/scale refs absent in the example config;
- model, document and scale profile templates remain `UNSELECTED` and disabled;
- no provider/runtime/tokenizer/document engine/topology/license is claimed selected;
- W10 owner and swarm packet sets match exactly: model provider, two workers, daemon, bridge,
  publication, pins, reclaimer and `search-eval`;
- `search-eval` W10 packet consumes accepted W9/P15 handoffs and cannot self-accept G6;
- forty-five probe templates exist: fifteen per candidate and three per each of five G6 evidence IDs;
- static probe templates remain `DISABLED` and contain no raw-output/reviewer evidence;
- configuration cannot activate optional depth and no optional transition is `APPLY_LIVE`;
- network, auto-download/update, training/learning, persistent input cache, generative authority,
  document execution/remote resources and active in-place schema changes remain forbidden;
- rerank output remains a subset of its input and provider output cannot become source evidence;
- dense/multivector/document/scale changes require candidate generation/migration;
- scale route switch remains a guarded redb transaction; alias movement is not commit;
- old routes are protected until route/epoch pins drain;
- removal restores accepted P15 behavior before optional physical reclaim;
- package writers cannot select providers, edit shared evidence, enable features/config or self-accept G6;
- all repository workflows remain manual-only and read-only.

## Still unproved

- real stable Rust toolchain/dependency set and committed `Cargo.lock`;
- contract/domain/port/configuration implementation and generated schemas;
- Windows root ownership, ACL, secret store and process containment;
- redb/CAS correctness and fault recovery;
- exact Qdrant server/client artifact and capability execution;
- lexical, Rust parser, regex and structural profile qualification;
- source watcher/USN, overlay non-persistence and currentness runtime behavior;
- access/IDF noninterference and exact source-backed query behavior;
- publication, migration, pin/reclaim, purge/restore and handle fault matrices;
- generic client pairing/binding/protocol/CLI runtime evidence;
- materialized Product Pulse corpus, exact A/B baselines, Windows environment and pre-registered policy;
- Product Pulse quality, latency, resource, recovery, stress and leakage evidence;
- any model/runtime/tokenizer/quantization or model-worker Windows qualification;
- any document provider/runtime/format/sandbox/fuzz/coordinate qualification;
- any measured one-shard bottleneck or advanced-scale topology qualification;
- optional incremental benefit, removal regression or G6 acceptance.

## Launch recommendation

Assign `search-contracts`; accept its canonical API/schema digest; then assign `search-domain` and
`search-ports` separately. Do not start W1+ because its stage/read-set packet exists.

A W10 candidate may be ticketed only after an exact accepted P15 report and reviewer receipt exist. The
ticket selects one candidate/profile, one dedicated ADR and exact immutable artifacts. Until then:

```text
model provider: disabled
model worker: absent/stopped
document provider: disabled
document worker: absent/stopped
advanced scale: disabled
candidate evaluation: blocked
accepted G6 candidates: none
baseline DIRECT/LEXICAL/CODE: authoritative candidate only, not yet implemented or accepted
```

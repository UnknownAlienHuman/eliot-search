# Implementation waves

The architecture authorizes implementation at **P00 only**. The scaffold makes later ownership visible
but does not authorize parallel implementation past the active gate. Launch a wave only after all
required dependency handoffs and the preceding gate evidence are accepted.

A package may reappear in a later wave for hardening or integration. Ownership does not move: the same
package boundary receives a new scoped assignment. Optional W10 work is prohibited before an accepted
P15 decision.

## W0 — Contract freeze

**Maps to:** P00 / G0

**Packages:** `search-contracts`, `search-domain`

**Goal:** Freeze vendor-neutral identities, epochs, recipes, reasons, views, memberships, grants, plans,
budgets, anchors, handles and pure invariant rules.

**Exit evidence:** Architecture hash check; exact v1 recipe set; canonical serialization fixtures;
dependency policy; pure property tests. No Qdrant, redb, watcher, parser or runtime dependency.

## W1 — Process, secrets and control shell

**Maps to:** P01–P02 / G1 foundation

**Packages:** `search-runtime-owner`, `search-os-secrets`, `search-control-redb`,
`search-provider-protocol (transport primitives)`, `eliot-searchd (shell)`, `eliot-search (shell)`

**Goal:** Establish one data-root owner, OS-user-bound opaque secret references, bounded control journal,
immutable snapshot publication, local frame codec and clean daemon/CLI lifecycle.

**Exit evidence:** Second-owner denial; crash/reopen owner proof; secret plaintext absent from config,
argv and logs; migration/corruption fixtures; hot queries write nothing; CLI opens no store.

## W2 — Direct source spine

**Maps to:** P03–P04 / G1

**Packages:** `search-source-admission`, `search-source-identity`, `search-source-registry`,
`search-safe-reader`, `search-revision-store`, `search-materializer (text/code baseline)`,
`search-unitizer`

**Goal:** Deliver one authoritative admission decision, source ownership, identity/path history, stable
no-execute reads, immutable residency-aware revisions, anchors and deterministic units without an index.

**Exit evidence:** Deny-by-default sensitive-source fixtures; rename/hardlink/reparse/unstable-read
fixtures; source-owner cutover state machine; exact revision readback; coordinate and residency proofs.

## W3 — Qualified lexical index

**Maps to:** P05–P07 / G2

**Packages:** `search-qdrant-supervisor`, `search-qdrant-bridge`, `search-point-identity`,
`search-lexical`, `search-projection-planner`, `search-epoch-pins`, `search-index-reclaimer`,
`search-publication`

**Goal:** Qualify one exact Qdrant artifact and lexical profile, isolate process ownership from the data
plane, build point manifests and linearizable epoch publication, and reclaim only point IDs below the
pin watermark.

**Exit evidence:** Executable/hash/PID/Job Object and secret-handling proof; capability fixture; no
implicit analyzer defaults; collision non-overwrite; one membership per point; every publication
failpoint; no pinned reclamation; reclaimer never uses broad correctness-path filters.

## W4 — Baseline query product

**Maps to:** P08 / G2 product slice

**Packages:** `search-access`, `search-query-planner`, `search-retrieval-executor`,
`search-candidate-validator`, `search-handles`, `search-result-projector`, `search-continuation`,
`search-eval (baseline harness)`

**Goal:** Implement locate/find_text through vendor-neutral index/readback ports with pre-candidate
access, bounded execution, source-backed validation, opaque handles, compact cards and continuations.

**Exit evidence:** Query crates have no direct Qdrant/redb/process dependency; access noninterference;
deterministic PlanFingerprint/order; bounded queues/results; source-backed candidates only; handle
authorization/TTL tests; raw read/grep baseline captured.

## W5 — Current workspace and code structure

**Maps to:** P09–P10 / G3 foundation

**Packages:** `search-source-reconcile`, `search-overlay`, `search-code-enricher`

**Goal:** Add complete observation reconciliation, saved/unsaved overlays and the qualified Rust
structural profile.

**Exit evidence:** Watcher overflow/resume; no false currentness across gaps; unsaved-byte
non-persistence; malformed/cfg parser fixtures; no compiler-certainty overclaim.

## W6 — Comparison and exact proof

**Maps to:** P11–P12 / G3

**Packages:** `search-subject-resolver`, `search-comparator`, `search-exact`

**Goal:** Resolve subjects without guessing, compare implementations by lineage/evidence role and
compile/execute frozen-denominator exact proofs through inventory/readback ports.

**Exit evidence:** Renamed analogue and false same-name fixtures; fork collapse; decisive
tests/config variants; complete-negative proof fails on drift/unreadable/cancelled scope; no indexed
top-k denominator.

## W7 — Security and lifecycle hardening

**Maps to:** P13

**Packages:** `search-retention`, `search-handles (durable hardening)`, `search-access (hardening)`,
`search-candidate-validator (hardening)`, `search-continuation (hardening)`,
`search-publication (restore/purge interaction)`, `search-revision-store (mark-and-sweep integration)`,
`search-index-reclaimer (purge interaction)`

**Goal:** Linearize restrictive revocation, durable-handle authorization, CAS mark-and-sweep, purge,
restore quarantine and ordinary index reclamation without transferring state ownership.

**Exit evidence:** Revocation during every query checkpoint; contaminated legs discarded; handle
denial; purge non-resurrection; resumable mark-and-sweep; mismatched restore quarantine; ordinary
reclamation and security purge remain distinct paths.

## W8 — Generic client edge

**Maps to:** P14 / G4

**Packages:** `search-provider-protocol (binding/integration)`,
`search-eliot-adapter (optional profile)`, `search-research-export-adapter (optional profile)`,
`eliot-searchd/eliot-search integration`

**Goal:** Complete authenticated binding, capability descriptors and generic evidence edges; enable
leaf compatibility profiles only when requested.

**Exit evidence:** Generic request→server plan→candidate round trip; no reverse authority or canonical
DB access; exact optional ELIOT/Research fixtures when enabled.

## W9 — Product Pulse and Windows qualification

**Maps to:** P15 / G5

**Packages:** `search-eval`, `eliot-searchd`, `eliot-search`

**Goal:** Run the full control corpus, latency/resource/fault/security qualification and decide whether
the lexical/code product is accepted.

**Exit evidence:** Raw A/B/C evidence; source-admission and leakage audit; protocol stress; recovery
matrix; explicit accepted/rejected verdict. Unit tests alone do not pass.

## W10 — Optional depth

**Maps to:** P16–P18 / G6

**Packages:** `search-model-provider`, `eliot-search-model-worker`, `eliot-search-doc-worker`,
`search-materializer (qualified document provider)`, existing publication/runtime owners for measured
scale migration

**Goal:** Add semantic, rerank, document or scale profiles only after accepted P15 evidence and a
dedicated ADR.

**Exit evidence:** Measured material benefit, exact artifact qualification, uninstall/removal fallback,
migration kill tests and rollback. No optional profile is baseline-required.

## Launch rule

For each package, the orchestrator reads `swarm/crates.toml`, verifies every direct dependency handoff,
creates one isolated worktree and gives the agent only the root/family/package instructions plus
accepted dependency API notes. The integration owner merges in topological order and records the exact
commit used by downstream agents.

# Implementation waves

The architecture authorizes implementation at **P00/W0 only**. Later ownership is visible but not
launchable until `swarm/launch-state.toml` advances with accepted evidence.

## W0 — Contract freeze

**Packages:** `search-contracts`, then `search-domain`.

Freeze vendor-neutral IDs, views, memberships, grants, plans, budgets, anchors, handles, recipes,
reason codes, port shapes and pure invariant functions.

**Exit:** architecture hash; exact recipe set; canonical serialization; port/schema digest; dependency
policy; pure property tests. No redb, Qdrant, watcher, parser or runtime dependency.

## W1 — Runtime, secrets and control shell

**Packages:** `search-runtime-owner`, `search-os-secrets`, `search-control-redb`,
`search-provider-protocol` transport primitives, daemon/CLI shells.

**Exit:** second-owner denial; crash/reopen owner proof; secret plaintext absent from side channels;
journal migration/corruption fixtures; hot query writes nothing; bounded frame/session shell.

## W2 — Direct source spine

**Packages:** `search-source-admission`, `search-source-identity`, `search-source-registry`,
`search-safe-reader`, `search-revision-store`, baseline `search-materializer`, `search-unitizer`.

**Exit:** deny-by-default admission receipts; rename/hardlink/reparse/unstable-read fixtures;
source-owner cutover; exact revision readback; coordinate/residency proofs.

## W3 — Qualified lexical index

**Packages:** `search-qdrant-supervisor`, `search-qdrant-bridge`, `search-point-identity`,
`search-lexical`, `search-projection-planner`, `search-epoch-pins`, `search-publication`,
`search-index-reclaimer`.

**Exit:** exact artifact/hash/PID/Job Object and secret handling proof; capability/schema fixture;
lexical golden vectors; collision non-overwrite; publication failpoint matrix; committed exact retired
manifest; no pinned reclaim and no broad correctness deletion.

## W4 — Baseline query product

**Packages:** `search-access`, `search-query-planner`, `search-retrieval-executor`,
`search-candidate-validator`, `search-handles`, `search-result-projector`, `search-continuation`,
`search-eval` baseline harness.

**Exit:** query crates have no concrete adapter edge; access/IDF noninterference; deterministic plans and
ordering; bounded queues/results; exact source-backed candidates; handle authorization/TTL; raw baseline captured.

## W5 — Current workspace and code structure

**Packages:** `search-source-reconcile`, `search-overlay`, `search-code-enricher`.

**Exit:** watcher overflow/resume; no false currentness across gaps; unsaved-byte non-persistence;
malformed/cfg parser fixtures; no compiler-certainty overclaim.

## W6 — Comparison and exact proof

**Packages:** `search-subject-resolver`, `search-comparator`, `search-exact`.

**Exit:** renamed analogue/false-name/fork fixtures; decisive tests/config variants; complete-negative
proof fails on drift, unreadable items or cancellation; inventory/readback remain port-driven.

## W7 — Security and lifecycle hardening

**Packages:** `search-retention`, durable `search-handles`, and hardening passes for access, validator,
continuation, publication, revision store and index reclaimer.

**Exit:** revocation at every checkpoint; contaminated legs discarded; handle denial; resumable CAS
mark/sweep; purge non-resurrection; restore quarantine; ordinary reclaim and purge receipts remain distinct.

## W8 — Generic client edge

**Packages:** provider-protocol binding/integration, optional ELIOT and Research leaf adapters, daemon/CLI integration.

**Exit:** generic request → server-owned plan → candidate round trip; no reverse authority or canonical
DB access; exact optional adapter fixtures when enabled.

## W9 — Product Pulse and Windows qualification

**Packages:** `search-eval`, daemon and CLI.

**Exit:** raw A/B/C evidence; latency/resource/fault/security report; source-admission/leakage audit;
protocol stress; explicit accepted/rejected verdict. Unit tests alone do not pass.

## W10 — Optional depth

Model, rerank, document or scale profiles only after accepted P15 evidence and dedicated ADR.

## Launch rule

For each package the orchestrator verifies launch authorization, accepted direct dependencies and port
handoffs, creates an isolated worktree, supplies only bounded instructions, enforces write scope and
merges in topological order with a wave receipt.

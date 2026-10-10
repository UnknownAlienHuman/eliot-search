# ELIOT Search repository instructions

## Product identity

ELIOT Search is a **standalone Qdrant-backed analysis and retrieval framework for agents and human operators**. The implementation target is Architecture 8.4: safe local source preparation, indexed candidate retrieval, exact verification and source-backed navigation across large repositories, multi-repository portfolios and qualified document/research corpora.

The supported product starts and operates without ELIOT Memory OS, a Governor, swarm controller, agent roles, assignment tickets or writer leases.

### Qdrant is the only indexed retrieval substrate

Use Qdrant for sparse/optional dense vectors, payload indexes, filtered retrieval, filtered IDF, count/scroll and exact point readback. Do not implement a second production search engine, inverted index, vector store or query database.

### Search owns

- source admission, identity, immutable revisions and no-execute acquisition;
- materialization, unitization, coordinate maps and structural enrichment;
- deterministic lexical encodings and separately qualified semantic profiles;
- projection manifests, collision-safe point identity, publication and rebuild;
- workspace/corpus/portfolio scope, lineage and currentness;
- exact/keyword/structural/sparse/optional semantic planning;
- access-scoped retrieval, deterministic fusion, candidate validation and exact readback;
- orientation, recommended reading, provenance, coverage and exact verification;
- standalone daemon/CLI and optional typed leaf adapters.

### Search does not own

- tasks, attempts, task history or finish authority;
- Governor/General Manager state or agent scheduling;
- mailboxes, steer/goal routing or model loops;
- assignment tickets, writer leases, approval/signature profiles;
- cross-project orchestration.

Those belong to `eliot-swarm-controller` and `eliot-memory-os`. Search may be called by them; it does not embed them.

## Product behavior

The intended workflow is:

```text
orient a bounded scope
→ retrieve likely evidence
→ inspect entities/relations
→ compare repositories or documents
→ verify important claims through exact scans
→ expand exact source-backed handles
```

Heuristic retrieval proposes candidates only. Every branch preserves assurance and coverage. Exact retained-source readback is mandatory before citations. A complete negative claim requires an exact frozen denominator and never follows from top-k saturation.

Agent results should minimize context use: compact ranked cards, exact handles, recommended reading, explicit ambiguity/coverage/freshness and bounded suggestions for the next exact check. Search provides evidence and navigation, not a task verdict.

The v1 contract lacks general free-text retrieval and first-class orientation. Do not fake them by overloading `find_text@1` or exposing raw Qdrant queries; use the versioned work tracked by #213.

## Authority order

Resolve conflicts in this order:

1. Part I of `docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`;
2. accepted ADRs, especially ADR 0005 and ADR 0006;
3. accepted public/provider/qualification contracts;
4. nearest package `AGENTS.md`, `FUNCTIONS.md` and package docs;
5. current maintainer request, exact issue/PR and dependency contract;
6. Cargo manifests and compiled source reality;
7. historical material under `swarm/**`, `docs/handoff/**`, `docs/execution/**` and Architecture Part II.

Part II and P00/ticket/lease material are historical scaffolding. They cannot authorize or block current product work. `swarm/launch-state.toml`, ticket drafts, context manifests, submissions and handoffs are repository history, not runtime inputs or implementation authority.

A maintainer request or exact issue plus one non-overlapping writer worktree is sufficient authorization. Do not build controller machinery inside Search. Residual controller tooling is removal work under #214.

## Queue authority and project maps

These sources have different jobs:

1. Architecture/ADRs/package contracts define product semantics.
2. `docs/product/EXECUTION_PROTOCOL.md` defines repository execution.
3. Programme issue #352, `docs/audit/AGENT_LAUNCH_GATE_2026-10-09.md`, the exact active issue/PR and current merged source define the **live writer queue**.
4. `docs/product/PROJECT_COMPLETION.md` is the long-range obligation/dependency graph. Its dated “next task”, base SHA or local stage wording is not live scheduling authority.
5. Audit files retain evidence and findings; they do not authorize source changes.

When the completion map is stale, follow #352, the launch gate, merged artifacts and the exact issue; update the map separately. Never repeat delivered work because an old row remains present.

## One-manager integration exceptions

Current process/addenda:

- `docs/product/EXECUTION_PROTOCOL.md` — mandatory bounded source slices, finding triage, review budgets and serial merge discipline;
- `docs/audit/WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md` — remaining Wave-2 order and subagent roles;
- `docs/audit/WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md` — exact issue/path exceptions;
- `docs/product/PROJECT_COMPLETION.md` — long-range implementation-to-release graph only.

Rules:

1. one manager, one writer worktree and one `Cargo.lock` integrator;
2. an exception applies only to one exact issue row, current base and frozen source slice;
3. subagents are read/research/review only;
4. all unlisted package prohibitions remain in force;
5. an exception cannot create a second authority, temporary public facade or product-reachable fallback;
6. every source slice records Definition of Ready fields and publishes `SCOPE_FROZEN` no later than its second source commit;
7. after freeze, findings are classified `B0`, `F1`, `F2`, `D` or `Q`; only an exact same-slice compile/safety B0 may remain in the PR;
8. work outside the frozen row becomes a follow-up or a **serial** split: merge tranche 1, publish the new `main`, then create tranche 2 from that new `main`; no stacked source PRs;
9. open PRs are merge candidates; programme/tracking/gate coordination lives in issues/docs, not permanent PRs.

The completion map and exception matrix do not grant blanket cross-package access. An integration slice changes only its named owner, explicit adapters and immediate reverse-consumer wiring.

## Work-item classes

```text
PROGRAM    broad capability tracker; issue/docs only, never a code branch
SLICE      one causal source result delivered by one mergeable PR
FOLLOW-UP  separately owned defect found outside the frozen slice
GATE       evidence/qualification against exact candidate bytes
```

Programme closure is not a source-merge prerequisite. A programme may remain open after a coherent slice merges.

## Scope freeze and review budget

Publish `SCOPE_FROZEN` by the second source commit with:

```text
base/head SHA
primary owner and state/effect
allowed production paths and narrow adapters
public/private API delta
immediate reverse consumers
persisted profile disposition
legacy deletion owner
minimum source gates
out-of-scope owners
```

Default split triggers:

```text
primary owners:              1
narrow adapter families:     <= 2
changed production files:    <= 30
changed production lines:    <= 2,500
persisted migrations:        <= 1
cross-owner cutovers:         <= 1
```

These triggers do not permit omitted correctness. Crossing one means stop and split sequentially or obtain a reviewed exception before additional source changes.

Finding taxonomy:

- `B0`: without the fix, the frozen result does not compile for a declared consumer, mints false authority, corrupts data, widens access/disclosure or enables an unsafe path that cannot remain unavailable/fail-closed;
- `F1`: same-owner follow-up;
- `F2`: adjacent-owner follow-up;
- `D`: unchanged baseline harness/lint/topology debt;
- `Q`: native/live/scale/release qualification.

A B0 claim names exact path, symbol, caller and causal chain. “Related to the programme” is not sufficient.

## Required working method

1. Read Architecture Part I, this file, `EXECUTION_PROTOCOL.md`, nearest package instructions and the exact issue.
2. Record exact base SHA, owner, paths, reverse consumers, non-goals, persisted-byte disposition, deletion boundary and minimum gates.
3. Use one writer worktree.
4. Freeze scope by the second source commit.
5. Fix product code first; do not replace implementation with planning/evidence prose.
6. Keep behavior in its actual owner. Cross-package composition belongs in a reviewed integration slice.
7. Reuse Qdrant through `search-qdrant-bridge`; do not build a parallel index.
8. Run the smallest meaningful gates:

   ```text
   cargo +1.98.0 check --locked -p <changed-owner> --all-features
   cargo +1.98.0 clippy --locked -p <changed-owner> --all-features -- -D warnings
   ```

   Add immediate reverse-consumer compilation for public API changes and focused causal fixtures only.
9. Capture broad known baseline failures once and compare once at final head. Identical unrelated debt is not a slice gate; new diagnostics are.
10. Obtain a formal review bound to the exact final head. A load-bearing commit expires earlier review.
11. Merge the coherent source slice when declared gates pass. Create the successor branch only from the newly merged `main`.
12. Never claim execution not performed at the exact revision.
13. Keep Actions manual-only; do not add push/PR triggers just to obtain a build.
14. Do not commit local paths, credentials, source contents or unredacted logs.
15. Keep the PR body as the current status record; do not post a long progress comment after every commit.

Read-only research for the next non-overlapping slice may run while the current PR is in final review, but no successor source branch/PR is opened before merge.

## Product invariants

- Qdrant is the only production indexed/search database.
- redb stores bounded technical control state, never a searchable corpus.
- Qdrant payload is projection metadata, never source evidence; candidates require exact source/revision validation.
- Retrieval, filtered IDF and exact denominator share one eligibility contract; denied data cannot influence ranking/statistics/counts/traces.
- Paths are locators, not identities. Native/source/revision/representation/projection identities remain distinct.
- Immutable revisions/manifests are never rewritten in place. Publication is generation/epoch safe.
- Unknown external mutation outcome is neither success nor blind-replay permission.
- Restrictive access/purge/shadow changes fail closed across retrieval, handles and continuations.
- Portfolio scopes are explicit and immutable; forks/mirrors/copies collapse by lineage.
- Scores from different access/scoring populations are not directly comparable; fusion is deterministic and versioned.
- Query admission is read-only and creates no ordinary durable query job/store.
- Standalone daemon/CLI and ELIOT adapter share one state/query authority.
- Optional model/document workers stay disabled until individually qualified.
- Vendor Qdrant types remain private to `search-qdrant-bridge`.
- Search never executes indexed source code, document macros or remote resources.
- An LLM is not required in the Search hot path and cannot replace source validation or coverage accounting.

## Large-corpus and document requirements

Large repositories and portfolios are first-class workloads. Use partitioned legs, progressive output, deterministic fusion, source/lineage caps and explicit budgets. Scale claims require #215 evidence.

Raw text/source/Git are baseline. Document/research support reuses the same source, preparation, projection, validation and coordinate owners. PDF/HTML/Markdown/LaTeX/Office/OCR profiles require separate #216 qualification; unsupported formats remain explicit.

## Standalone and ELIOT integration

Standalone operation is baseline, not fallback. The daemon owns installation/root/process and serves the local client without an external controller.

ELIOT integration is a leaf translation layer over accepted provider protocols. It receives no canonical ELIOT credentials, task authority or finish authority. No configuration may make Memory OS or the swarm controller mandatory for normal startup, ingestion, indexing, search, rebuild or recovery.

## Evidence and acceptance

Compilation is necessary but not product qualification. Product claims require exact named evidence:

- real Qdrant artifact/client/schema/query/restart probes;
- native Windows identity, containment, secrets and named-pipe evidence;
- real source → durable state → Qdrant → validated result;
- source-backed orientation/recommended reading/exact checks;
- multi-repository scale, quality and resource evidence;
- document coordinate/citation evidence for enabled profiles;
- restart/recovery/deny/unknown-outcome/rebuild cases;
- installed baseline evidence before release.

Planning records, issue status, source presence, mock/oracle success, signatures and review badges cannot substitute for those gates.

## GitHub connector use

Before concluding GitHub is read-only, load the full unfiltered GitHub catalog, confirm `permissions.push == true`, and use API write actions. A harmless unattached blob may be used as a write probe. Do not infer capabilities from a filtered list or missing local Git credentials.

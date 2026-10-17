# ELIOT Search repository instructions

## Product identity

ELIOT Search is a **standalone Qdrant-backed analysis and retrieval framework for agents and human operators**. The implementation target is Architecture 8.4: safe local source preparation, indexed candidate retrieval, exact verification and source-backed navigation across large repositories, multi-repository portfolios and qualified document/research corpora.

The supported product starts and operates without ELIOT Memory OS, a Governor, swarm controller, agent roles, assignment tickets or writer leases.

### Qdrant is the only indexed retrieval substrate

Use Qdrant for sparse/optional dense vectors, payload indexes, filtered retrieval, filtered IDF, count/scroll and exact point readback. Do not implement a second production search engine, inverted index, vector store or query database.

### Search owns

- source admission, identity, immutable revisions and no-execute acquisition;
- materialization, unitization, coordinate maps and structural enrichment;
- deterministic document/query lexical encoding and separately qualified semantic profiles;
- projection manifests, collision-safe point identity, publication and rebuild;
- workspace/corpus/portfolio scope, repository lineage and currentness;
- exact, keyword, structural, sparse lexical and optional semantic query planning;
- access-scoped retrieval, deterministic fusion, candidate validation and exact source readback;
- corpus orientation, recommended reading, provenance, coverage and exact verification;
- standalone daemon/CLI and optional typed provider adapters.

### Search does not own

- Tasks, WorkScopes, Attempts or canonical task history;
- General Manager/Governor state, agent scheduling or native harness lifecycle;
- mailboxes, steer/goal routing, model loops or subagent observation;
- generic assignment-ticket issuance, writer leases, approval/signature profiles;
- review acceptance, finish authority or cross-project orchestration.

Those responsibilities belong to `eliot-swarm-controller` and, after integration, `eliot-memory-os`. Search may be called by those systems; it does not embed or reimplement them.

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

Heuristic retrieval proposes candidates only. Sparse lexical, structural, keyword, optional dense and rerank branches preserve assurance and coverage. Exact retained-source readback is mandatory before citations. A complete negative claim requires an exact frozen denominator and never follows from top-k saturation.

Agent results should minimize context use: compact ranked cards, exact handles, recommended reading, explicit ambiguity/coverage/freshness and bounded suggestions for the next exact `grep`, regex, symbol or structural check. Search provides evidence and navigation, not a task verdict.

The v1 contract lacks general free-text retrieval and first-class orientation. Do not fake them by overloading `find_text@1` or exposing raw Qdrant queries; use the versioned work tracked by #213.

## Authority order

Resolve conflicts in this order:

1. Part I of `docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`;
2. accepted ADRs, especially ADR 0005 and ADR 0006;
3. accepted public contracts, provider schemas and product qualification contracts;
4. `docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md` (mandatory remediation of verified code/security/spine defects);
5. nearest package `AGENTS.md`, `FUNCTIONS.md` and package-owned documentation;
6. current maintainer request, exact issue/PR and dependency contract;
7. Cargo manifests and compiled source reality;
8. historical material under `swarm/**`, `docs/handoff/**`, `docs/execution/**` and Architecture Part II.

Part II and P00/ticket/lease material are historical scaffolding. They cannot authorize or block current product work. `swarm/launch-state.toml`, ticket drafts, context manifests, leases, submissions, reviews and handoffs are repository history, not runtime inputs or implementation authority.

A maintainer request or exact ready issue plus the sole active writer worktree is sufficient authorization, subject to the live queue. Only one source branch/PR may be active; each successor starts from newly merged `main`. Do not build controller machinery inside Search. Residual controller tooling is removal work under #214.

## Mandatory anti-slop and code-quality rules (2026-10-17 audit mandate)

Read `docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md` before starting any slice. All agents must enforce these rules without exception:

1. **No fake cryptography or synthetic digests:**
   Never implement custom FNV-1a (`0xcbf2_9ce4_8422_2325`), SplitMix64, XOR-folding or hand-rolled SHA-256 (`0x428a_2f98`). Never wrap non-BLAKE3 output or repeated byte constants (`[0_u8; 32]`, `[0xA5; 32]`, `[0xA1; 32]`, `[0x6c; 32]`) into `Blake3Digest32`. Add `blake3.workspace = true` (or `sha2.workspace = true`) to the package's `Cargo.toml` and compute real domain-separated cryptographic digests with length-prefixed fields or canonical CBOR (`search_contracts::canonical::to_canonical_cbor`).
2. **Never grow `xtask/canonical-digest-ledger.json` or `swarm/coverage/**`:**
   Adding new exemptions to `xtask/canonical-digest-ledger.json` or building new `xtask` AST/coverage police to excuse unfinished code is forbidden. Fix the fake digest/receipt in the owning crate and delete its ledger exemptions.
3. **Cut over and delete legacy duplicates in the same slice:**
   Do not leave broken legacy types or prototypes (such as the 16-byte FNV `PointIdentityDigest([u8; 16])` and `ProjectionKind` point key in `search-point-identity`, the in-memory `bins/eliot-searchd/src/lexical/` index, `src/snapshot/`, `src/sealed_*`, `src/main.rs`, or `src/bin/eliot-search-sealed-*.rs`) sitting beside their replacements. Cross-package slices that update a crate and its immediate reverse consumers to **delete** the broken implementation are explicitly authorized and required.
4. **No dead-code facades or unwired daemon modules:**
   Do not land new `#[allow(dead_code)]` composition modules in `bins/eliot-searchd`. `bins/eliot-searchd` must enable `full-baseline` by default and wire `src/entry.rs::main()` through the real Architecture 8.4 spine (`search-source-admission` / `search-safe-reader` → `search-materializer` / `search-unitizer` → `search-control-redb` → `search-projection-planner` / `search-point-identity` / `search-qdrant-bridge` / `search-publication` → `search-access` / `search-query-planner` / `search-retrieval-executor` / `search-candidate-validator` → `search-provider-protocol`).
5. **No tautological security guards or production test fixtures:**
   Never pass `path.parent()` as `admitted_root` to `safe_reader_adapter`. Never hardcode `ReadSecurityDisposition::Permitted`, barrier revision `1`, or `[0xA5; 32]` vault receipts in production code. Never silently downgrade Windows root/handle identity to path bytes without explicit downgraded identity classification. Keep `Fake*` structs (`FakeGuards`) and `build_test_*` helpers strictly inside `#[cfg(test)]` or `tests/`.

## Queue authority and project maps

These sources have different jobs:

1. Architecture/ADRs/package contracts define product semantics.
2. `docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md` defines the mandatory code-repair and spine-wiring priorities.
3. `docs/product/EXECUTION_PROTOCOL.md` defines repository execution.
4. Programme issue #352, `docs/audit/AGENT_LAUNCH_GATE_2026-10-09.md`, the exact active issue/PR and current merged source define the **live writer queue**.
5. `docs/product/PROJECT_COMPLETION.md` is the long-range obligation/dependency graph. Its dated “next task”, base SHA or local stage wording is not live scheduling authority.

When the completion map is stale, follow #352, the 2026-10-17 audit remediation phases, merged artifacts and the exact issue; update the map separately. Never repeat delivered work because an old row remains present.

## One-manager integration exceptions

Current process/addenda:

- `docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md` — mandatory removal of fake digests, legacy duplicates, security bypasses and unwired daemon modules;
- `docs/product/EXECUTION_PROTOCOL.md` — bounded source slices, finding triage, review budgets and serial merge discipline;
- `docs/audit/WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md` — remaining Wave-2 order and subagent roles;
- `docs/audit/WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md` — exact issue/path exceptions;
- `docs/product/PROJECT_COMPLETION.md` — long-range implementation-to-release graph only.

Rules:

1. one manager, one writer worktree and one `Cargo.lock` integrator;
2. a slice may touch a primary owner plus its immediate reverse consumers when cutting over and deleting a broken legacy API, fake digest or unwired daemon adapter;
3. subagents are read/research/review only;
4. all unlisted package prohibitions remain in force;
5. an exception cannot create a second authority, temporary public facade or product-reachable fallback;
6. every source slice records Definition of Ready fields and publishes `SCOPE_FROZEN` no later than its second source commit;
7. after freeze, findings are classified `B0`, `F1`, `F2`, `D` or `Q`; only an exact same-slice compile/safety B0 may remain in the PR;
8. work outside the frozen row becomes a follow-up or a **serial** split: merge tranche 1, publish the new `main`, then create tranche 2 from that new `main`; no stacked source PRs;
9. open PRs are merge candidates; programme/tracking/gate coordination lives in issues/docs, not permanent PRs.

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
primary owners:              1 (or owner + immediate reverse consumers when deleting legacy/fake APIs)
narrow adapter families:     <= 2
changed production files:    <= 30
changed production lines:    <= 2,500
persisted migrations:        <= 1
cross-owner cutovers:        <= 1
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

1. Read Architecture Part I, this file, `docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md`, `EXECUTION_PROTOCOL.md`, nearest package instructions and the exact issue.
2. Record exact base SHA, owner, paths, reverse consumers, non-goals, persisted-byte disposition, deletion boundary and minimum gates.
3. Use one writer worktree created from the current `origin/main`.
4. Freeze scope by the second source commit.
5. Fix product code first; do not replace implementation with planning/evidence prose or exemption-ledger entries.
6. Keep behavior in its actual owner. Cross-package composition belongs in a reviewed integration slice.
7. Reuse Qdrant through `search-qdrant-bridge`; do not build a parallel index.
8. Run the smallest meaningful gates:

   ```text
   cargo +1.98.0 check --locked -p <changed-owner> --all-features
   cargo +1.98.0 clippy --locked -p <changed-owner> --all-features -- -D warnings
   ```

   Select production targets (`--lib`/`--bins` as applicable), including immediate reverse consumers and `eliot-searchd --all-features` when daemon wiring is touched.
9. Reuse recorded unrelated baseline debt. If a production gate fails, compare its relevant base/candidate diagnostics once. New candidate production diagnostics block.
10. Obtain a formal review bound to the exact final head, with no unresolved B0/P1 and explicit F1/F2/D/Q dispositions. A load-bearing commit expires earlier review. In a single-owner repository, a formal COMMENT review may record the independent reviewer result and manager disposition; self-approval is not required.
11. Merge the coherent source slice when declared gates pass. Create the successor branch only from the newly merged `main`.
12. Never claim execution not performed at the exact revision.
13. Keep Actions manual-only; do not add push/PR triggers just to obtain a build.
14. Do not commit local paths, credentials, API keys, source contents or unredacted logs.
15. Keep the PR body as the current status record; do not post a long progress comment after every commit.

Read-only research for the next non-overlapping slice may run while the current PR is in final review, but no successor source branch/PR is opened before merge.

PROCESS/DOCS PRs change no product source, manifests, lockfile or workflows. Any source-changing PR is a SLICE and obeys the sole-source-PR rule regardless of label.

## Product invariants

- Qdrant is the only indexed/search database. No SQLite/FTS, Tantivy, Lucene, local postings database or alternate production search engine.
- redb stores bounded technical control state, never a second searchable corpus.
- Qdrant payload is projection metadata, never source evidence. Candidates require authoritative exact source/revision readback and validation.
- Retrieval, filtered IDF population, exact denominator, count, scroll, facets, grouping, clusters, recommend/discover and response validation use one eligibility base filter. Denied data cannot influence permitted candidate visibility, ranking, statistics, counts, facets, groups, clusters, recommendations, suggestions or traces.
- Paths are locators, not identities. File/native identity, source identity, revision identity, representation identity and projection identity remain distinct.
- Immutable revisions and manifests are never rewritten in place. Publication is generation/epoch safe.
- Unknown external mutation outcome is not success and is not blindly replayed.
- Restrictive access/purge/shadow changes fail closed across retrieval, handles and continuations.
- Multi-repository portfolios are explicit immutable scopes; forks/mirrors/copies collapse by lineage and cannot masquerade as independent evidence.
- Raw scores from different access/scoring populations are not directly comparable; cross-leg fusion is deterministic and versioned.
- Query admission is read-only and creates no ordinary durable query job or second query-state store.
- The standalone daemon/CLI and ELIOT adapter share one underlying state and query authority.
- Optional model/document workers remain disabled until independently qualified and are not required for the first lexical/code baseline.
- Vendor Qdrant types stay inside `search-qdrant-bridge`; public boundaries use Eliot-owned types.
- Search never executes indexed source code, document macros or remote resources.
- An LLM is not required in the Search hot path. A consuming agent may interpret typed results but cannot replace source validation, exact proof or coverage accounting.

## Large-corpus and document requirements

Large repositories and repository portfolios are first-class workloads, not optional demos. Implementation remains bounded through partitioned legs, progressive output, deterministic fusion, per-source/per-lineage caps and explicit query budgets. Scale claims require the measured qualification in #215.

Raw text, source code and Git are the first baseline. Research/document support reuses the same source, materialization, projection, validation and exact-coordinate owners. PDF/HTML/Markdown/LaTeX/Office/OCR profiles require separate qualification under #216; unsupported formats remain explicit.

## Standalone and ELIOT integration

Standalone operation is the baseline, not a fallback. The daemon owns its installation/root/process and serves the public local client contract without any external controller.

ELIOT integration is a leaf translation layer over accepted provider protocols. Search returns bounded candidate/result records, coverage, freshness, assurance, reason codes and opaque handles. It does not receive canonical ELIOT credentials, task authority, Context Compiler admission or finish authority.

No Search configuration option may make ELIOT Memory OS or `eliot-swarm-controller` mandatory for normal startup, ingestion, indexing, search, rebuild or recovery.

## Repository metadata boundary

Static package maps and ownership crosswalks may help avoid overlapping edits. They cannot issue authority or block work. Do not extend the active tree with generic:

- assignment/lease event state machines;
- context materialization for agent prompts;
- approval/signature/profile registries;
- task/attempt/mailbox/scheduler records;
- orchestrator credentials or actor roles;
- controller qualification suites;
- large captures proving only repository-control tooling.

Reusable controller work belongs in `UnknownAlienHuman/eliot-swarm-controller`. Canonical ELIOT task and memory authority belongs in `UnknownAlienHuman/eliot-memory-os`.

## Evidence and acceptance

Compilation is necessary but not product qualification. Product claims require exact named evidence:

- real Qdrant artifact/client/schema/query/restart probes for indexed capability;
- native Windows identity, containment, secrets and named-pipe evidence;
- real source → durable state → Qdrant → validated result for the product spine;
- agent retrieval/orientation fixtures with source-backed recommended reading and exact checks;
- multi-repository scale, quality and resource evidence;
- document coordinate/citation evidence for enabled materializers;
- restart/recovery, deny, unknown-outcome and rebuild cases;
- installed baseline evidence before release.

Planning records, ticket status, source presence, mock/oracle success, signatures and review badges cannot substitute for those gates.

## GitHub connector use

Before concluding GitHub is read-only, load the full unfiltered GitHub catalog, confirm `permissions.push == true`, and use API write actions. A harmless unattached blob may be used as a write probe. Do not infer capabilities from a filtered list or missing local Git credentials.

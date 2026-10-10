# ELIOT Search repository instructions

## Product identity

ELIOT Search is a **standalone Qdrant-backed analysis and retrieval framework for agents and human
operators**. The implementation target is Architecture 8.4: safe local source preparation, indexed
candidate retrieval, exact verification and source-backed navigation across large repositories,
multi-repository portfolios and qualified document/research corpora.

The supported product must start and operate without ELIOT Memory OS, a Governor, a swarm controller,
agent roles, assignment tickets or writer leases.

### Qdrant is the retrieval substrate

Use Qdrant for the indexed projection: sparse/optional dense vectors, payload indexes, filtered retrieval,
filtered IDF, count/scroll and exact point readback. Do not reimplement its physical search engine or add
another production index.

### Search owns the analysis framework around Qdrant

- source admission, identity, immutable revisions and no-execute acquisition;
- materialization, unitization, coordinate maps and structural enrichment;
- deterministic document/query lexical encoding and optional qualified semantic profiles;
- projection manifests, collision-safe point identity, publication and rebuild;
- workspace/corpus/portfolio scope, repository lineage and currentness;
- exact, keyword, structural, sparse lexical and optional semantic query planning;
- access-scoped retrieval, deterministic fusion, candidate validation and source readback;
- corpus orientation, recommended reading, provenance, coverage and exact verification;
- standalone daemon/CLI and optional typed provider adapters.

### Not owned here

- Tasks, WorkScopes, Attempts or canonical task history;
- General Manager/Governor state, agent scheduling or native harness lifecycle;
- mailboxes, steer/goal routing, model loops or subagent observation;
- generic assignment-ticket issuance, writer leases, approval/signature profiles;
- review acceptance, finish authority or cross-project orchestration.

Those responsibilities belong to `eliot-swarm-controller` and, after integration, `eliot-memory-os`.
Search may be called by those systems; it must not embed or reimplement them.

## Product behavior

The intended agent workflow is:

```text
orient a bounded scope
→ retrieve likely evidence
→ inspect entities/relations
→ compare repositories or documents
→ verify important claims through exact scans
→ expand exact source-backed handles
```

Heuristic search is allowed and required, but it proposes candidates only. Sparse lexical, structural,
keyword, optional dense and rerank branches must preserve their assurance and coverage. Exact source
readback is mandatory before citations are emitted. A complete negative claim requires an exact frozen
denominator and never follows from top-k saturation.

Agent-facing results should minimize context consumption: compact ranked cards, exact handles,
recommended reading, explicit ambiguity/coverage/freshness and bounded suggestions for the next exact
`grep`, regex, symbol or structural check. Search provides evidence and navigation, not a task verdict.

The current v1 contract lacks general free-text retrieval and first-class scope orientation; do not fake
those features by overloading `find_text@1` or accepting raw Qdrant queries. Implement the versioned
contract tracked by #213.

## Authority order

Resolve conflicts in this order:

1. **Part I** of `docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`;
2. accepted product ADRs, especially ADR 0005 and ADR 0006;
3. accepted public contracts, provider schemas and product qualification contracts;
4. nearest package `AGENTS.md`, `FUNCTIONS.md` and package-owned documentation;
5. the current maintainer request, issue/PR acceptance criteria and exact dependency contracts;
6. Cargo manifests and compiled source reality;
7. historical planning material under `swarm/**`, `docs/handoff/**`, `docs/execution/**` and Part II of
   the implementation master.

Part II (`Codex Handoff 2.7`) is historical implementation scaffolding. Its P00-only sequencing and any
linked ticket/lease machinery do not authorize or block current product work. ADR 0005/0006 and current
product issues supersede those repository-process instructions without weakening Part I product
invariants.

`swarm/launch-state.toml`, ticket drafts, context manifests, leases, submissions, reviews and handoffs are
**advisory or historical repository metadata only**. They are not product inputs. Their presence, absence
or status cannot authorize, block or widen implementation. Any lower-precedence text saying otherwise is
superseded.

A maintainer request, issue or PR plus one non-overlapping branch/worktree is sufficient authorization to
work. Do not build a ticket issuer, lease service, PKI, role database or other controller inside Search.
Residual controller tooling is removal work under #214, not a product feature to complete.

## Current one-manager integration exceptions

The accepted Wave packets may incorporate a **narrow issue-specific addendum** to the nearest package
instructions when one manager must move existing code between packages or integrate an exact root
dependency/`Cargo.lock` change. This does not lower the package boundary generally.

Current accepted addenda:

- `docs/audit/WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md` for `#237/#253`;
- `docs/audit/WAVE2_SINGLE_MANAGER_PACKET_2026-10-09.md` for the post-`#237` serialized Wave 2;
- `docs/audit/WAVE2_PACKAGE_EXCEPTION_MATRIX_2026-10-09.md` for exact issue/path exceptions;
- `docs/product/PROJECT_COMPLETION.md` for the remaining named implementation and integration phases,
  using only each phase's explicit owned files/call-site wiring and current detailed issue scope.

Rules:

1. one manager and one writer worktree only;
2. an exception applies only to the exact issue row and latest accepted base published by `#97`;
3. subagents are read/research/review only and never edit repository state;
4. the manager remains the sole root dependency and `Cargo.lock` integrator;
5. all unlisted package prohibitions remain in force;
6. an exception cannot introduce a second authority, temporary public facade or product-reachable
   compatibility implementation;
7. if required work falls outside the row, stop and amend the owner issue/packet before editing.

The whole-project map is a phase/dependency and donor-reading guide, not blanket cross-package access.
Its programme/gate rows authorize no source changes. An integration phase may change only its named
owner, explicitly listed boundary adapters and actual reverse-consumer wiring. No new runtime controller
or generic task/approval registry is authorized. Existing required legacy APIs may remain unchanged only
until their named consumer cutover/removal owner; they cannot mint or substitute new-profile authority.

This mechanism exists to make a package extraction or exact dependency cutover possible without telling
agents to violate the nearest package instructions or granting broad cross-package permission.

## Required working method

1. Read Architecture Part I, these root instructions, the nearest package instructions and named issue/PR
   before editing.
2. Use one branch/worktree per manager and one active writer per overlapping package scope.
3. Fix product code first. Do not replace missing implementation with planning documents, registries,
   schemas or evidence prose.
4. Keep changes within the actual owning package. Cross-package composition belongs in the daemon or a
   separately reviewed integration branch, except for an exact current addendum above.
5. Use the existing Qdrant capabilities through `search-qdrant-bridge`; do not build a parallel inverted
   index, vector store, filter engine or query database.
6. Run the smallest meaningful compilation gate after each implementation slice:

   ```text
   cargo +1.98.0 check --locked -p <package> --all-features
   cargo +1.98.0 clippy --locked -p <package> --all-features -- -D warnings
   ```

   Use applicable target flags when the package is library-only. Full product/process tests are deferred
   until implementation is complete unless a change requires an immediate focused proof.
7. Never claim execution that was not performed at the exact revision.
8. Keep GitHub Actions manual-only. Do not add push/pull-request triggers merely to obtain a build.
9. Do not commit generated local paths, credentials, API keys, source contents or unredacted logs.

## Product invariants

- Qdrant is the only indexed/search database. No SQLite/FTS, Tantivy, Lucene, local postings database or
  alternate production search engine.
- redb stores bounded technical control state, never a second searchable corpus.
- Qdrant payload is projection metadata, never source evidence. Candidates require authoritative exact
  source/revision readback and validation.
- Retrieval, filtered IDF population and exact denominator use one eligibility contract. Denied data must
  not influence permitted ranking, statistics, counts, grouping, clusters, suggestions or traces.
- Paths are locators, not identities. File/native identity, source identity, revision identity,
  representation identity and projection identity remain distinct.
- Immutable revisions and manifests are never rewritten in place. Publication is generation/epoch safe.
- Unknown external mutation outcome is not success and is not blindly replayed.
- Restrictive access/purge/shadow changes fail closed across retrieval, handles and continuations.
- Multi-repository portfolios are explicit immutable scopes; forks/mirrors/copies are collapsed by
  lineage and cannot masquerade as independent evidence.
- Raw scores from different access/scoring populations are not directly comparable; cross-leg fusion is
  deterministic and versioned.
- Query admission is read-only and creates no ordinary durable query job or second query-state store.
- The standalone daemon/CLI and ELIOT adapter share one underlying state and query authority.
- Optional model/document workers remain disabled until independently qualified and are not required for
  the first lexical/code baseline.
- Vendor Qdrant types stay inside `search-qdrant-bridge`; public boundaries use Eliot-owned types.
- Search never executes indexed source code, document macros or remote resources.
- An LLM is not required in the Search hot path. A consuming agent may interpret typed results but cannot
  replace source validation, exact proof or coverage accounting.

## Large-corpus and document requirements

Large repositories and repository portfolios are first-class product workloads, not optional demos.
Implementation must remain bounded through partitioned legs, progressive output, deterministic fusion,
per-source/per-lineage caps and explicit query budgets. Scale claims require the measured qualification in
#215.

Raw text, source code and Git are the first baseline. Research/document support must reuse the same source,
materialization, projection, validation and exact-coordinate owners. PDF/HTML/Markdown/LaTeX/Office/OCR
profiles require separate qualification under #216; unsupported formats remain explicit.

## Standalone and ELIOT integration

Standalone operation is the baseline, not a fallback. The daemon owns its installation/root/process and
serves the public local client contract without any external controller.

ELIOT integration is a leaf translation layer over accepted provider protocols. Search returns bounded
candidate/result records, coverage, freshness, assurance, reason codes and opaque handles. It does not
receive canonical ELIOT credentials, task authority, Context Compiler admission or finish authority.

No Search configuration option may make ELIOT Memory OS or `eliot-swarm-controller` mandatory for normal
startup, ingestion, indexing, search, rebuild or recovery.

## Repository metadata boundary

Static package maps and ownership crosswalks may help avoid overlapping edits. They cannot issue authority
or block work. Do not extend the active tree with generic:

- assignment/lease event state machines;
- context materialization for agent prompts;
- approval/signature/profile registries;
- task/attempt/mailbox/scheduler records;
- orchestrator credentials or actor roles;
- controller qualification suites;
- large captures proving only repository-control tooling.

Reusable controller work belongs in `UnknownAlienHuman/eliot-swarm-controller`. Canonical ELIOT task and
memory authority belongs in `UnknownAlienHuman/eliot-memory-os`.

## Evidence and acceptance

Compilation is necessary but not product qualification. Product claims require exact named evidence:

- real Qdrant artifact/client/schema/query/restart probes for indexed capability;
- native platform evidence for Windows identity, containment, secrets and named pipes;
- real source → durable state → Qdrant → validated result for the product spine;
- agent retrieval/orientation fixtures with source-backed recommended reading and exact checks;
- multi-repository scale, quality and resource evidence;
- document coordinate/citation evidence for enabled materializers;
- restart/recovery, deny, unknown-outcome and rebuild cases;
- installed baseline evidence before release.

Planning records, ticket status, source presence and mock/oracle success cannot substitute for those gates.

## GitHub connector use

Before concluding GitHub is read-only, load the full unfiltered GitHub tool catalog, confirm
`permissions.push == true`, and use GitHub API write actions. A harmless unattached blob may be used as a
write probe. Do not infer capabilities from a filtered tool list or missing local Git credentials.